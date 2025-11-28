use eframe::egui::{self, Color32, Pos2, Rect, Stroke};
use crate::{audio::AudioAnalysis, VisualizerConfig};
use std::f32::consts::TAU;

pub struct UnknownPleasuresVisualizer {
    last_amplitudes: Vec<f32>,
}

impl UnknownPleasuresVisualizer {
    pub fn new() -> Self {
        Self { last_amplitudes: Vec::new() }
    }

    /// Draw 80 horizontal lines, each representing a sub-frequency band of the
    /// spectrum. Creates a faux-3D perspective by scaling and offsetting farther
    /// lines.
    pub fn draw(&mut self, ui: &mut egui::Ui, rect: Rect, audio: &AudioAnalysis, cfg: &VisualizerConfig, time: f64) {
        let painter = ui.painter();
        let bands = cfg.up_max_lines as usize;
        let freq_len = audio.frequency_data.len().max(1);

        // Number of samples across each line (polyline resolution)
        let samples = cfg.up_samples as usize;

        // Ensure smoothing buffer length
        if self.last_amplitudes.len() < bands {
            self.last_amplitudes.resize(bands, 0.0);
        }

    // Horizontal span
    let width = rect.width();

        // Precompute a time-based phase for simple animation
        let phase = (time as f32) * 2.0;

        let center = rect.center();
        for i in 0..bands {
            let z = i as f32 / bands as f32; // 0..1 depth

            // Log-like frequency mapping using exponent curve to bias low frequencies
            let f0 = (i as f32) / (bands as f32);
            let f1 = ((i + 1) as f32) / (bands as f32);
            let exp = cfg.up_freq_curve_exponent.max(0.001);
            let idx0 = ((f0.powf(exp)) * (freq_len as f32)).floor() as usize;
            let idx1 = ((f1.powf(exp)) * (freq_len as f32)).floor() as usize;
            let start = idx0.min(freq_len - 1);
            let mut end = idx1.min(freq_len);
            if end <= start { end = (start + 1).min(freq_len); }

            // Average amplitude for this band (0.0..1.0)
            let mut sum = 0u32;
            for b in start..end {
                sum += audio.frequency_data[b] as u32;
            }
            let base_amp = (sum as f32) / ((end - start) as f32 * 255.0);
            // Apply audio-reactivity multipliers (bass/mid/treble)
            let raw_amp = base_amp * (1.0
                + cfg.up_bass_mult * audio.smooth_bass
                + cfg.up_mid_mult * audio.smooth_mid
                + cfg.up_treble_mult * audio.smooth_treble);

            // Temporal smoothing to reduce jitter
            let last = self.last_amplitudes[i];
            let smoothing = cfg.up_smoothing.clamp(0.0, 1.0);
            let amp = last + (raw_amp - last) * smoothing;
            self.last_amplitudes[i] = amp;

            // Perspective scaling and offsets
            let perspective = 1.0 - z * cfg.up_perspective; // closer lines larger
            let line_thickness = (cfg.up_line_thickness * perspective).max(0.3);
            let alpha = (200.0 * (1.0 - z)).max(40.0) as u8;

            // Baseline for this line: spread vertically but push "far" lines up to create depth
            let spacing = rect.height() / (bands as f32 * 0.9);
            let baseline = rect.bottom() - (i as f32 * spacing) + z * (rect.height() * -0.2);

            // Vertical amplitude scale
            let amp_scale = cfg.up_vertical_scale * 100.0 * amp * perspective;

            // Color mode: monochrome (white) or tinted using base_color
            let color = if cfg.up_monochrome {
                Color32::from_rgba_unmultiplied(255, 255, 255, alpha)
            } else {
                let base = cfg.base_color;
                Color32::from_rgba_unmultiplied(base.r(), base.g(), base.b(), alpha)
            };

            // Generate polyline points
            let mut prev: Option<Pos2> = None;
            // Rotation angle in radians (if enabled)
            let angle_rad = cfg.up_rotation_deg.to_radians();
            let (ca, sa) = if cfg.up_isometric_rotate { (angle_rad.cos(), angle_rad.sin()) } else { (1.0f32, 0.0f32) };
            for s in 0..samples {
                let t = s as f32 / (samples - 1) as f32;
                // local x centered around 0 so rotation/zoom happens around center
                let local_x = (t - 0.5) * width * cfg.up_line_length * cfg.up_zoom;

                // create a waveform-like shape using a sine carrier modulated by amplitude
                let freq_mod = 1.0 + (z * 6.0);
                let carrier = (t * TAU * freq_mod + phase * (1.0 + z)).sin();

                // small jitter based on t to break perfect symmetry
                let jitter = ((t * 50.0).sin() * 0.15 + (t * 12.0).cos() * 0.08) * (1.0 - z) * 0.6;

                let local_y = (baseline - center.y) - (carrier * amp_scale * (1.0 + jitter));

                // Apply isometric rotation and translation back to center
                let rx = local_x * ca - local_y * sa;
                let ry = local_x * sa + local_y * ca;
                let pt = Pos2::new(center.x + rx, center.y + ry);

                if let Some(p0) = prev {
                    painter.line_segment([p0, pt], Stroke::new(line_thickness, color));
                }
                prev = Some(pt);
            }
        }
    }
}
