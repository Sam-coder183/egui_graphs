use egui::{Context, Window, Slider};

pub struct Settings3D {
    pub rotation_x: f32,
    pub rotation_y: f32,
    pub rotation_z: f32,
    pub auto_rotate: bool,
    pub rotation_speed: f32,
    pub perspective_strength: f32,
    pub show_settings: bool,
}

impl Settings3D {
    pub fn default() -> Self {
        Self {
            rotation_x: 30.0,
            rotation_y: 45.0,
            rotation_z: 0.0,
            auto_rotate: false,
            rotation_speed: 0.5,
            perspective_strength: 0.001,
            show_settings: false,
        }
    }

    pub fn show(&mut self, ctx: &Context) {
        if self.show_settings {
            let mut open = true;
            Window::new("3D Visualization Settings")
                .open(&mut open)
                .resizable(true)
                .show(ctx, |ui| {
                    ui.heading("Rotation");
                    ui.add(Slider::new(&mut self.rotation_x, -180.0..=180.0).text("X Axis"));
                    ui.add(Slider::new(&mut self.rotation_y, -180.0..=180.0).text("Y Axis"));
                    ui.add(Slider::new(&mut self.rotation_z, -180.0..=180.0).text("Z Axis"));
                    
                    ui.separator();
                    
                    ui.checkbox(&mut self.auto_rotate, "Auto Rotate");
                    if self.auto_rotate {
                        ui.add(Slider::new(&mut self.rotation_speed, 0.0..=5.0).text("Speed"));
                    }
                    
                    ui.separator();
                    
                    ui.heading("Perspective");
                    ui.add(Slider::new(&mut self.perspective_strength, 0.0..=0.01).text("Strength"));
                    
                    ui.separator();
                    
                    ui.horizontal(|ui| {
                        if ui.button("Reset").clicked() {
                            self.rotation_x = 30.0;
                            self.rotation_y = 45.0;
                            self.rotation_z = 0.0;
                            self.perspective_strength = 0.001;
                        }
                    });
                });
            if !open {
                self.show_settings = false;
            }
        }
    }
    
    pub fn update(&mut self, ctx: &Context) {
        if self.auto_rotate {
            self.rotation_y += self.rotation_speed;
            if self.rotation_y > 180.0 {
                self.rotation_y -= 360.0;
            }
            ctx.request_repaint();
        }
    }
}
