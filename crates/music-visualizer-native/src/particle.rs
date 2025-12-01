use eframe::egui::{Color32, Pos2, Vec2};

#[derive(Clone)]
pub struct Particle {
    pub pos: Pos2,
    pub vel: Vec2,
    pub life: f32,
    pub max_life: f32,
    pub size: f32,
    pub color: Color32,
}

impl Particle {
    pub fn new(center: Pos2, angle: f32, speed: f32, color: Color32) -> Self {
        Self {
            pos: center,
            vel: Vec2::new(angle.cos() * speed, angle.sin() * speed),
            life: 1.0,
            max_life: 1.0,
            size: 3.0 + rand::random::<f32>() * 5.0,
            color,
        }
    }
    pub fn update(&mut self, dt: f32) {
        self.pos += self.vel * dt;
        self.vel *= 0.98;
        self.life -= dt / self.max_life;
    }
    pub fn is_alive(&self) -> bool {
        self.life > 0.0
    }
}
