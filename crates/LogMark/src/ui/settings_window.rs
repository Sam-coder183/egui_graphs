use egui::{Context, Window, Slider};

pub struct SettingsWindow {
    pub open: bool,
    pub font_size_edge_label: f32,
    pub ui_scale: f32,
}

impl Default for SettingsWindow {
    fn default() -> Self {
        Self {
            open: false,
            font_size_edge_label: 14.0,
            ui_scale: 1.0,
        }
    }
}

impl SettingsWindow {
    pub fn show(&mut self, ctx: &Context) {
        if self.open {
            Window::new("Settings")
                .open(&mut self.open)
                .show(ctx, |ui| {
                    ui.heading("Appearance");
                    ui.add(Slider::new(&mut self.ui_scale, 0.5..=2.0).text("UI Scale"));
                    if ui.button("Apply UI Scale").clicked() {
                        ctx.set_pixels_per_point(self.ui_scale);
                    }
                    
                    ui.separator();
                    ui.heading("Graph Text");
                    ui.add(Slider::new(&mut self.font_size_edge_label, 8.0..=30.0).text("Edge Label Size"));
                });
        }
    }
}
