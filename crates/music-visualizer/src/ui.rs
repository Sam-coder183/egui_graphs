use eframe::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};
use crate::{MusicVisualizerApp, TreeConfig, FrameState};

// HSL to RGB color conversion (moved here with UI code)
pub fn hsl_to_rgb(h: f32, s: f32, l: f32) -> Color32 {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h * 6.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;

    let (r, g, b) = match (h * 6.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };

    Color32::from_rgb(
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
    )
}

impl MusicVisualizerApp {
    // Draw fractal and helpers (moved from lib.rs)
    pub fn get_current_color(&self) -> Color32 {
        if self.config.color_cycle {
            let hue = (self.time as f32 * self.config.color_cycle_speed) % 1.0;
            hsl_to_rgb(hue, 0.8, 0.6)
        } else {
            self.config.base_color
        }
    }

    pub fn draw_fractal(&self, ui: &mut egui::Ui, rect: Rect) {
        let painter = ui.painter();
        
        // Draw background with beat flash (once for the scene)
        let bg_intensity = (self.beat_flash * 30.0) as u8;
        let bg = Color32::from_rgb(
            self.config.background_color.r().saturating_add(bg_intensity),
            self.config.background_color.g().saturating_add(bg_intensity / 2),
            self.config.background_color.b().saturating_add(bg_intensity),
        );
        painter.rect_filled(rect, 0.0, bg);

        // Draw trails (Blur)
        if self.config.blur_enabled {
            for (i, state) in self.history.iter().enumerate() {
                // Calculate opacity based on age (older = more transparent)
                // i=0 is newest (after current), i=len-1 is oldest
                // Actually push_front means 0 is newest.
                // We want to draw oldest first? No, order doesn't matter much for additive blending, 
                // but for alpha blending, back-to-front is usually better.
                // Let's draw oldest to newest.
                
                let age_factor = 1.0 - (i as f32 / self.history.len() as f32);
                let opacity = self.config.blur_opacity * age_factor;
                
                if opacity > 0.01 {
                    for tree in &self.config.trees {
                        self.draw_single_tree_state(ui, rect, tree, state, opacity, self.config.blur_depth_reduction);
                    }
                }
            }
        }

        // Draw all trees (Current State)
        for tree in &self.config.trees {
            self.draw_single_tree(ui, rect, tree);
        }
    }

    pub fn draw_single_tree(&self, ui: &mut egui::Ui, rect: Rect, tree_config: &TreeConfig) {
        // Use current state
        let state = FrameState {
            audio: self.audio.clone(),
            rotation: self.rotation,
            time: self.time,
        };
        self.draw_single_tree_state(ui, rect, tree_config, &state, 1.0, 0);
    }

    pub fn draw_single_tree_state(&self, ui: &mut egui::Ui, rect: Rect, tree_config: &TreeConfig, state: &FrameState, opacity_mult: f32, depth_reduction: u32) {
        let painter = ui.painter();
        let center = rect.center() + tree_config.origin_offset;

        // Calculate reactive parameters using the provided state
        let (bass, _mid, treble, volume) = if let Some(band_idx) = tree_config.frequency_band {
             // We need to implement get_band_amplitude for the state's audio
             // Since get_band_amplitude is on self, we can't easily call it on state.audio without refactoring.
             // But we can duplicate the logic here or extract it.
             // Let's duplicate for now as it's short.
             let freq_len = state.audio.frequency_data.len().max(1);
             let f0 = (band_idx as f32) / 16.0;
             let f1 = ((band_idx + 1) as f32) / 16.0;
             let exp = self.config.up_freq_curve_exponent.max(0.001);
             let idx0 = ((f0.powf(exp)) * (freq_len as f32)).floor() as usize;
             let idx1 = ((f1.powf(exp)) * (freq_len as f32)).floor() as usize;
             let start = idx0.min(freq_len - 1);
             let mut end = idx1.min(freq_len);
             if end <= start { end = (start + 1).min(freq_len); }
             let mut sum = 0.0;
             for i in start..end {
                 if i < state.audio.frequency_data.len() {
                     sum += state.audio.frequency_data[i] as f32;
                 }
             }
             let count = (end - start).max(1) as f32;
             let amp = (sum / count) / 255.0;
             (amp, amp, amp, amp)
        } else {
             (state.audio.smooth_bass, state.audio.smooth_mid, state.audio.smooth_treble, state.audio.smooth_volume)
        };

        let zoom = tree_config.base_zoom + bass * tree_config.zoom_bass_mult;
        let width = tree_config.base_width + bass * tree_config.width_bass_mult;
        let depth = (tree_config.base_depth as f32
            + state.audio.spectral_centroid * tree_config.depth_complexity_mult) as u32;
        let brightness = tree_config.base_brightness
            + treble * tree_config.brightness_treble_mult;

        // Clip drawing to rect
        let clip_rect = rect;

        // Calculate base length to fit within the rect (use smaller dimension)
        let max_size = rect.width().min(rect.height()) * 0.35;
        let base_length = max_size * zoom;
        let branch_angle = std::f32::consts::PI / 4.0 * width;
        
        // Color from state time
        let hue = if self.config.color_cycle {
            (state.time as f32 * self.config.color_cycle_speed) % 1.0
        } else {
            // We don't have easy access to base_color hue, so just use current config color
            // If color_cycle is off, color is static anyway.
            0.0 // Dummy, handled below
        };
        
        let mut color = if self.config.color_cycle {
            hsl_to_rgb(hue, 0.8, 0.6)
        } else {
            self.config.base_color
        };
        
        // Apply opacity
        if opacity_mult < 1.0 {
            color = Color32::from_rgba_unmultiplied(
                color.r(), color.g(), color.b(), 
                (255.0 * opacity_mult) as u8
            );
        }

        // Draw glow effect at center first (behind fractal) - Only for main tree (opacity 1.0) to save perf
        if self.config.glow_intensity > 0.0 && opacity_mult > 0.9 {
            let glow_color = Color32::from_rgba_unmultiplied(
                color.r(),
                color.g(),
                color.b(),
                (self.config.glow_intensity * volume * 100.0) as u8,
            );
            let glow_radius = (base_length * 0.5 * (1.0 + bass)).min(max_size * 0.6);
            painter.circle_filled(center, glow_radius, glow_color);
        }

        // Draw fractal tree starting from center, growing upward
        // Apply tree-specific angle offset
        let base_angle = -std::f32::consts::PI / 2.0 + state.rotation * 0.1 + tree_config.angle_offset.to_radians();
        
        // Reduce depth for trails
        let effective_depth = depth.saturating_sub(depth_reduction);
        
        self.draw_branch(
            painter,
            center,
            base_length,
            base_angle,
            branch_angle,
            effective_depth,
            tree_config.base_depth as f32,
            brightness,
            color,
            clip_rect,
            tree_config.pseudo_3d,
            tree_config.tilt_x,
            tree_config.tilt_y,
        );
    }

    pub fn draw_branch(
        &self,
        painter: &egui::Painter,
        start: Pos2,
        length: f32,
        angle: f32,
        branch_angle: f32,
        depth: u32,
        max_depth: f32,
        brightness: f32,
        color: Color32,
        clip_rect: Rect,
        pseudo_3d: bool,
        tilt_x: f32,
        tilt_y: f32,
    ) {
        if depth == 0 || length < 2.0 {
            return;
        }

        let mut end = Pos2::new(
            start.x + angle.cos() * length,
            start.y + angle.sin() * length,
        );
        
        // Apply pseudo-3D tilt if enabled
        if pseudo_3d {
            // Simple perspective projection simulation
            // We assume the tree grows in a 2D plane, and we tilt that plane.
            // The 'y' coordinate in the tree's local space (growing up) corresponds to Z in 3D space if we tilt back.
            // But here we are just offsetting the end point based on its relative position to start?
            // No, we should transform the vector (end - start).
            
            let dx = end.x - start.x;
            let dy = end.y - start.y;
            
            // Apply tilt:
            // tilt_x affects x based on y (shear)
            // tilt_y affects y based on x (shear)
            // This is a simple shear transformation which can look like perspective/3D rotation
            
            let new_dx = dx + dy * (tilt_x * 0.01);
            let new_dy = dy + dx * (tilt_y * 0.01);
            
            end = Pos2::new(start.x + new_dx, start.y + new_dy);
        }

        // Skip if both points are outside the clip rect
        if !clip_rect.contains(start) && !clip_rect.contains(end) {
            // Check if line might still cross the rect
            let line_rect = Rect::from_two_pos(start, end);
            if !line_rect.intersects(clip_rect) {
                return;
            }
        }

        // Vary color based on depth
        let depth_factor = depth as f32 / max_depth;
        let line_color = Color32::from_rgba_unmultiplied(
            (color.r() as f32 * brightness * depth_factor) as u8,
            (color.g() as f32 * brightness * depth_factor) as u8,
            (color.b() as f32 * brightness * depth_factor) as u8,
            (255.0 * depth_factor) as u8,
        );

        let stroke_width = (depth as f32 * 0.1).max(0.5);
        painter.line_segment([start, end], Stroke::new(stroke_width, line_color));

        // Audio-reactive branch angles
        let angle_mod = self.audio.smooth_mid * 0.2;

        // Recursive branches
        let new_length = length * (0.65 + self.audio.smooth_treble * 0.1);

        self.draw_branch(painter, end, new_length, angle - branch_angle + angle_mod,
            branch_angle * 0.95, depth - 1, max_depth, brightness, color, clip_rect, pseudo_3d, tilt_x, tilt_y);
        self.draw_branch(painter, end, new_length, angle + branch_angle - angle_mod,
            branch_angle * 0.95, depth - 1, max_depth, brightness, color, clip_rect, pseudo_3d, tilt_x, tilt_y);
    }

    pub fn draw_spectrum(&self, ui: &mut egui::Ui, rect: Rect) {
        let painter = ui.painter();
        let bar_count = 64;
        let bar_width = rect.width() / bar_count as f32;

        for i in 0..bar_count {
            let idx = i * self.audio.frequency_data.len() / bar_count;
            let value = if idx < self.audio.frequency_data.len() {
                self.audio.frequency_data[idx] as f32 / 255.0
            } else {
                0.0
            };

            let height = value * rect.height();
            let x = rect.left() + i as f32 * bar_width;
            let bar_rect = Rect::from_min_max(
                Pos2::new(x, rect.bottom() - height),
                Pos2::new(x + bar_width - 1.0, rect.bottom()),
            );

            let hue = i as f32 / bar_count as f32;
            let color = hsl_to_rgb(hue, 0.8, 0.5);
            painter.rect_filled(bar_rect, 0.0, color);
        }
    }

    pub fn draw_waveform(&self, ui: &mut egui::Ui, rect: Rect) {
        let painter = ui.painter();

        let points: Vec<Pos2> = self.audio.time_data.iter()
            .enumerate()
            .map(|(i, &v)| {
                let x = rect.left() + (i as f32 / self.audio.time_data.len() as f32) * rect.width();
                let y = rect.center().y + ((v as f32 - 128.0) / 128.0) * rect.height() * 0.5;
                Pos2::new(x, y)
            })
            .collect();

        if points.len() > 1 {
            for i in 0..points.len() - 1 {
                let hue = i as f32 / points.len() as f32;
                let color = hsl_to_rgb(hue, 0.7, 0.6);
                painter.line_segment([points[i], points[i + 1]], Stroke::new(2.0, color));
            }
        }
    }

    pub fn draw_particles(&mut self, painter: &egui::Painter, center: Pos2) {
        use crate::ParticleShape;
        
        for p in &mut self.particles {
            let life_factor = p.life / p.max_life;
            let alpha = (p.max_alpha * life_factor * 255.0) as u8;
            let color = Color32::from_rgba_unmultiplied(p.color.r(), p.color.g(), p.color.b(), alpha);
            
            // p.pos is now an offset from center (Vec2)
            let pos = center + p.pos;
            let size = p.size * life_factor;
            
            match p.shape {
                ParticleShape::Circle => {
                    painter.circle_filled(pos, size, color);
                },
                ParticleShape::Square => {
                    painter.rect_filled(Rect::from_center_size(pos, Vec2::splat(size * 2.0)), 0.0, color);
                },
                ParticleShape::Triangle => {
                    // Equilateral triangle
                    let r = size * 1.5;
                    let angle = -std::f32::consts::PI / 2.0; // Point up
                    let p1 = pos + Vec2::new(angle.cos() * r, angle.sin() * r);
                    let p2 = pos + Vec2::new((angle + 2.0 * std::f32::consts::PI / 3.0).cos() * r, (angle + 2.0 * std::f32::consts::PI / 3.0).sin() * r);
                    let p3 = pos + Vec2::new((angle + 4.0 * std::f32::consts::PI / 3.0).cos() * r, (angle + 4.0 * std::f32::consts::PI / 3.0).sin() * r);
                    
                    // Use PathShape for filled polygon
                    let points = vec![p1, p2, p3];
                    painter.add(egui::Shape::convex_polygon(points, color, Stroke::NONE));
                },
                ParticleShape::Star => {
                    // 5-pointed star
                    let outer_r = size * 1.5;
                    let inner_r = size * 0.6;
                    let mut points = Vec::new();
                    let start_angle = -std::f32::consts::PI / 2.0;
                    
                    for i in 0..10 {
                        let angle = start_angle + i as f32 * std::f32::consts::PI / 5.0;
                        let r = if i % 2 == 0 { outer_r } else { inner_r };
                        points.push(pos + Vec2::new(angle.cos() * r, angle.sin() * r));
                    }
                    
                    painter.add(egui::Shape::convex_polygon(points, color, Stroke::NONE));
                },
                ParticleShape::Hexagon => {
                    let r = size * 1.2;
                    let mut points = Vec::new();
                    for i in 0..6 {
                        let angle = i as f32 * std::f32::consts::PI / 3.0;
                        points.push(pos + Vec2::new(angle.cos() * r, angle.sin() * r));
                    }
                    painter.add(egui::Shape::convex_polygon(points, color, Stroke::NONE));
                }
            }
        }
    }

    // The full settings UI is implemented at crate root (lib.rs). We avoid
    // duplicating that long block here; keep drawing helpers (fractal,
    // spectrum, waveform, particles) in this module instead.
}
