use eframe::egui::{self, Color32, Pos2, Rect, Stroke};
use crate::app::MusicVisualizerNativeApp;

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

impl MusicVisualizerNativeApp {
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
        let center = rect.center();
        let zoom = self.config.base_zoom + self.audio.smooth_bass * self.config.zoom_bass_mult;
        let width = self.config.base_width + self.audio.smooth_bass * self.config.width_bass_mult;
        let depth = (self.config.base_depth as f32 + self.audio.spectral_centroid * self.config.depth_complexity_mult) as u32;
        let brightness = self.config.base_brightness + self.audio.smooth_treble * self.config.brightness_treble_mult;
        let bg_intensity = (self.beat_flash * 30.0) as u8;
        let bg = Color32::from_rgb(
            self.config.background_color.r().saturating_add(bg_intensity),
            self.config.background_color.g().saturating_add(bg_intensity / 2),
            self.config.background_color.b().saturating_add(bg_intensity),
        );
        painter.rect_filled(rect, 0.0, bg);
        let clip_rect = rect;
        let max_size = rect.width().min(rect.height()) * 0.35;
        let base_length = max_size * zoom;
        let branch_angle = std::f32::consts::PI / 4.0 * width;
        let color = self.get_current_color();
        if self.config.glow_intensity > 0.0 {
            let glow_color = Color32::from_rgba_unmultiplied(
                color.r(), color.g(), color.b(), (self.config.glow_intensity * self.audio.smooth_volume * 100.0) as u8,
            );
            let glow_radius = (base_length * 0.5 * (1.0 + self.audio.smooth_bass)).min(max_size * 0.6);
            painter.circle_filled(center, glow_radius, glow_color);
        }
        self.draw_branch(
            painter, center, base_length,
            -std::f32::consts::PI / 2.0 + self.rotation * 0.1,
            branch_angle, depth, brightness, color, clip_rect,
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
        brightness: f32,
        color: Color32,
        clip_rect: Rect,
    ) {
        if depth == 0 || length < 2.0 {
            return;
        }
        let end = Pos2::new(
            start.x + angle.cos() * length,
            start.y + angle.sin() * length,
        );
        if !clip_rect.contains(start) && !clip_rect.contains(end) {
            let line_rect = Rect::from_two_pos(start, end);
            if !line_rect.intersects(clip_rect) {
                return;
            }
        }
        let depth_factor = depth as f32 / self.config.base_depth as f32;
        let line_color = Color32::from_rgba_unmultiplied(
            (color.r() as f32 * brightness * depth_factor) as u8,
            (color.g() as f32 * brightness * depth_factor) as u8,
            (color.b() as f32 * brightness * depth_factor) as u8,
            (255.0 * depth_factor) as u8,
        );
        let stroke_width = (depth as f32 * 0.1).max(0.5);
        painter.line_segment([start, end], Stroke::new(stroke_width, line_color));
        let angle_mod = self.audio.smooth_mid * 0.2;
        let new_length = length * (0.65 + self.audio.smooth_treble * 0.1);
        self.draw_branch(painter, end, new_length, angle - branch_angle + angle_mod,
            branch_angle * 0.95, depth - 1, brightness, color, clip_rect);
        self.draw_branch(painter, end, new_length, angle + branch_angle - angle_mod,
            branch_angle * 0.95, depth - 1, brightness, color, clip_rect);
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
        for p in &mut self.particles {
            let alpha = (p.life * 255.0) as u8;
            let color = Color32::from_rgba_unmultiplied(p.color.r(), p.color.g(), p.color.b(), alpha);
            let pos = Pos2::new(
                center.x + (p.pos.x - 400.0),
                center.y + (p.pos.y - 300.0),
            );
            painter.circle_filled(pos, p.size * p.life, color);
        }
    }
    // Drawing helpers (fractal, spectrum, waveform, particles) have been integrated into the native UI module.
}
