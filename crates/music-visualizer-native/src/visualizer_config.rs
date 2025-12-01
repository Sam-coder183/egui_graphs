use eframe::egui::Color32;

#[derive(Clone)]
pub struct VisualizerConfig {
    pub base_zoom: f32,
    pub base_width: f32,
    pub base_depth: u32,
    pub base_brightness: f32,
    pub zoom_bass_mult: f32,
    pub width_bass_mult: f32,
    pub depth_complexity_mult: f32,
    pub brightness_treble_mult: f32,
    pub rotation_beat_mult: f32,
    pub auto_rotate: bool,
    pub rotation_speed: f32,
    pub pulse_on_beat: bool,
    pub color_cycle: bool,
    pub color_cycle_speed: f32,
    pub base_color: Color32,
    pub accent_color: Color32,
    pub background_color: Color32,
    pub glow_intensity: f32,
    pub particle_count: u32,
    pub up_line_thickness: f32,
    pub up_perspective: f32,
    pub up_vertical_scale: f32,
    pub up_line_length: f32,
    pub up_zoom: f32,
    pub up_isometric_rotate: bool,
    pub up_rotation_deg: f32,
    pub up_bass_mult: f32,
    pub up_mid_mult: f32,
    pub up_treble_mult: f32,
    pub up_max_lines: u32,
    pub up_samples: u32,
    pub up_freq_curve_exponent: f32,
    pub up_monochrome: bool,
    pub up_smoothing: f32,
}

impl Default for VisualizerConfig {
    fn default() -> Self {
        Self {
            base_zoom: 0.1,
            base_width: 1.0,
            base_depth: 16,
            base_brightness: 0.8,
            zoom_bass_mult: 0.1,
            width_bass_mult: 0.3,
            depth_complexity_mult: 4.0,
            brightness_treble_mult: 0.4,
            rotation_beat_mult: 0.1,
            auto_rotate: true,
            rotation_speed: 1.0,
            pulse_on_beat: true,
            color_cycle: true,
            color_cycle_speed: 0.1,
            base_color: Color32::from_rgb(100, 200, 255),
            accent_color: Color32::from_rgb(255, 100, 200),
            background_color: Color32::from_rgb(10, 10, 20),
            glow_intensity: 0.5,
            particle_count: 50,
            up_line_thickness: 1.5,
            up_perspective: 0.6,
            up_vertical_scale: 1.0,
            up_line_length: 1.0,
            up_zoom: 1.0,
            up_isometric_rotate: false,
            up_rotation_deg: 15.0,
            up_bass_mult: 1.2,
            up_mid_mult: 0.6,
            up_treble_mult: 0.2,
            up_max_lines: 80,
            up_samples: 120,
            up_freq_curve_exponent: 2.5,
            up_monochrome: true,
            up_smoothing: 0.15,
        }
    }
}

impl VisualizerConfig {
    /// Preset tuned to mimic the classic 'Unknown Pleasures' look (monochrome stacked spectra)
    pub fn preset_unknown_pleasures_image() -> Self {
        let mut c = Self::default();
        // Strong monochrome contrast
        c.base_color = Color32::WHITE;
        c.background_color = Color32::from_rgb(6, 6, 10);
        c.accent_color = Color32::from_rgb(200, 200, 200);

        // Unknown Pleasures tuned params
        c.up_monochrome = true;
        c.up_line_thickness = 2.0;
        c.up_perspective = 0.75;
        c.up_vertical_scale = 1.6;
        c.up_line_length = 1.5;
        c.up_zoom = 1.05;
        c.up_rotation_deg = 12.0;
        c.up_max_lines = 80;
        c.up_samples = 180;
        c.up_freq_curve_exponent = 3.2;
        c.up_smoothing = 0.22;

        // Reduce other visual distractions
        c.pulse_on_beat = false;
        c.color_cycle = false;
        c
    }

    /// Reset only the fractal-related parameters to their default values
    pub fn reset_fractal_to_default(&mut self) {
        let d = VisualizerConfig::default();
        self.base_zoom = d.base_zoom;
        self.base_width = d.base_width;
        self.base_depth = d.base_depth;
        self.base_brightness = d.base_brightness;

        self.zoom_bass_mult = d.zoom_bass_mult;
        self.width_bass_mult = d.width_bass_mult;
        self.depth_complexity_mult = d.depth_complexity_mult;
        self.brightness_treble_mult = d.brightness_treble_mult;
        self.rotation_beat_mult = d.rotation_beat_mult;
    }
}
