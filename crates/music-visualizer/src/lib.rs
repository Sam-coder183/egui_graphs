use eframe::egui::{self, Color32, Pos2, Rect, Vec2};
use std::cell::RefCell;
use std::rc::Rc;
use std::collections::VecDeque;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
mod audio;
mod ui;
mod unknown_pleasures;
use crate::audio::{AudioAnalysis, WebAudio, init_web_audio};
use crate::unknown_pleasures::UnknownPleasuresVisualizer;

// Playlist track information
#[derive(Clone, Default)]
pub struct PlaylistTrack {
    pub name: String,
    pub duration: f64,      // Duration in seconds
    pub file_type: String,  // mp3, wav, ogg, flac, etc.
    pub url: String,        // Object URL or external URL
}

// Playlist and playback state
#[derive(Clone)]
pub struct PlaylistState {
    pub tracks: Vec<PlaylistTrack>,
    pub current_index: Option<usize>,
    pub is_playing: bool,
    pub is_shuffled: bool,
    pub shuffle_order: Vec<usize>,
    pub current_time: f64,
    pub duration: f64,
    pub volume: f32,
}

impl Default for PlaylistState {
    fn default() -> Self {
        Self {
            tracks: Vec::new(),
            current_index: None,
            is_playing: false,
            is_shuffled: false,
            shuffle_order: Vec::new(),
            current_time: 0.0,
            duration: 0.0,
            volume: 0.8,
        }
    }
}

impl PlaylistState {
    pub fn get_current_track(&self) -> Option<&PlaylistTrack> {
        self.current_index.and_then(|idx| self.tracks.get(idx))
    }
    
    pub fn get_progress(&self) -> f32 {
        if self.duration > 0.0 {
            (self.current_time / self.duration) as f32
        } else {
            0.0
        }
    }
    
    pub fn format_time(seconds: f64) -> String {
        let mins = (seconds / 60.0) as u32;
        let secs = (seconds % 60.0) as u32;
        format!("{:02}:{:02}", mins, secs)
    }
    
    pub fn shuffle_playlist(&mut self) {
        let len = self.tracks.len();
        if len == 0 {
            return;
        }
        
        self.shuffle_order = (0..len).collect();
        // Simple Fisher-Yates shuffle using our rand function
        for i in (1..len).rev() {
            let j = (rand_float() * (i + 1) as f32) as usize;
            self.shuffle_order.swap(i, j);
        }
    }
    
    pub fn get_next_index(&self) -> Option<usize> {
        let len = self.tracks.len();
        if len == 0 {
            return None;
        }
        match self.current_index {
            Some(idx) => {
                if self.is_shuffled && !self.shuffle_order.is_empty() {
                    let current_shuffle_pos = self.shuffle_order.iter().position(|&x| x == idx)?;
                    if current_shuffle_pos + 1 < self.shuffle_order.len() {
                        Some(self.shuffle_order[current_shuffle_pos + 1])
                    } else {
                        None
                    }
                } else {
                    if idx + 1 < len {
                        Some(idx + 1)
                    } else {
                        None
                    }
                }
            }
            None => Some(0),
        }
    }
    
    pub fn get_prev_index(&self) -> Option<usize> {
        let len = self.tracks.len();
        if len == 0 {
            return None;
        }
        match self.current_index {
            Some(idx) => {
                if self.is_shuffled && !self.shuffle_order.is_empty() {
                    let current_shuffle_pos = self.shuffle_order.iter().position(|&x| x == idx)?;
                    if current_shuffle_pos > 0 {
                        Some(self.shuffle_order[current_shuffle_pos - 1])
                    } else {
                        None
                    }
                } else {
                    if idx > 0 {
                        Some(idx - 1)
                    } else {
                        None
                    }
                }
            }
            None => Some(0),
        }
    }
}

// Audio logic moved to `src/audio.rs`.

// Snapshot of state for motion blur/trails
#[derive(Clone)]
pub struct FrameState {
    pub audio: AudioAnalysis,
    pub rotation: f32,
    pub time: f64,
}

// Configuration for a single fractal tree
#[derive(Clone)]
pub struct TreeConfig {
    pub name: String,
    pub base_zoom: f32,
    pub base_width: f32,
    pub base_depth: u32,
    pub base_brightness: f32,
    pub zoom_bass_mult: f32,
    pub width_bass_mult: f32,
    pub depth_complexity_mult: f32,
    pub brightness_treble_mult: f32,
    pub angle_offset: f32,
    pub frequency_band: Option<usize>,
    pub origin_offset: Vec2,
    pub pseudo_3d: bool,
    pub tilt_x: f32,
    pub tilt_y: f32,
}

impl Default for TreeConfig {
    fn default() -> Self {
        Self {
            name: "Main Tree".to_string(),
            base_zoom: 0.3,
            base_width: 1.0,
            base_depth: 16,
            base_brightness: 0.8,
            zoom_bass_mult: 0.1,
            width_bass_mult: 0.3,
            depth_complexity_mult: 4.0,
            brightness_treble_mult: 0.4,
            angle_offset: 0.0,
            frequency_band: None,
            origin_offset: Vec2::ZERO,
            pseudo_3d: false,
            tilt_x: 0.0,
            tilt_y: 0.0,
        }
    }
}

// Configuration for visualizer
#[derive(Clone)]
pub struct VisualizerConfig {
    // Trees
    pub trees: Vec<TreeConfig>,
    
    // Animation
    pub auto_rotate: bool,
    pub rotation_speed: f32,
    pub rotation_beat_mult: f32,
    pub pulse_on_beat: bool,
    pub color_cycle: bool,
    pub color_cycle_speed: f32,
    
    // Visual style
    pub base_color: Color32,
    pub accent_color: Color32,
    pub background_color: Color32,
    pub glow_intensity: f32,
    pub particle_count: u32,
    
    // Sparkle Settings
    pub sparkle_count_on_beat: u32,
    pub sparkle_spawn_mode: SparkleSpawnMode,
    pub sparkle_radius: f32,
    pub sparkle_polygon_sides: u32,
    pub sparkle_size_min: f32,
    pub sparkle_size_max: f32,
    pub sparkle_speed_min: f32,
    pub sparkle_speed_max: f32,
    pub sparkle_grid_dims: Vec2, // X, Y spacing
    pub sparkle_min_dist: f32,
    
    // Continuous Sparkles
    pub sparkle_always_on: bool,
    pub sparkle_spawn_rate: f32, // Particles per second
    pub sparkle_life_min: f32,
    pub sparkle_life_max: f32,
    pub sparkle_alpha_min: f32,
    pub sparkle_alpha_max: f32,
    
    // Blur / Trails
    pub blur_enabled: bool,
    pub blur_samples: usize, // Number of past frames to draw
    pub blur_opacity: f32,   // Opacity of trails
    pub blur_depth_reduction: u32, // Reduce recursion depth for trails
    
    // Unknown Pleasures visualizer parameters
    pub up_line_thickness: f32,
    pub up_perspective: f32,
    pub up_vertical_scale: f32,
    pub up_line_length: f32,
    pub up_zoom: f32,
    pub up_isometric_rotate: bool,
    pub up_rotation_deg: f32,
    // Audio reactivity multipliers for Unknown Pleasures
    pub up_bass_mult: f32,
    pub up_mid_mult: f32,
    pub up_treble_mult: f32,
    // Additional Unknown Pleasures controls
    pub up_max_lines: u32,
    pub up_samples: u32,
    pub up_freq_curve_exponent: f32,
    pub up_monochrome: bool,
    pub up_smoothing: f32,
}

impl Default for VisualizerConfig {
    fn default() -> Self {
        // Create a default forest layout
        let trees = {
            let mut t = Vec::new();
            
            // Central tree
            let mut t1 = TreeConfig::default();
            t1.name = "Center Tree".to_string();
            t.push(t1);
            
            // Surrounding trees to cover the screen
            let offsets = [
                (Vec2::new(-350.0, -250.0), 45.0, "Top Left"),
                (Vec2::new(350.0, -250.0), -45.0, "Top Right"),
                (Vec2::new(-350.0, 250.0), 135.0, "Bottom Left"),
                (Vec2::new(350.0, 250.0), -135.0, "Bottom Right"),
                (Vec2::new(-400.0, 0.0), 90.0, "Left"),
                (Vec2::new(400.0, 0.0), -90.0, "Right"),
            ];

            for (pos, angle, name) in offsets {
                let mut tree = TreeConfig::default();
                tree.name = name.to_string();
                tree.origin_offset = pos;
                tree.base_zoom = 0.25; // Slightly smaller than center
                tree.angle_offset = angle;
                t.push(tree);
            }
            t
        };

        Self {
            trees,
            
            auto_rotate: true,
            rotation_speed: 1.0,
            rotation_beat_mult: 0.1,
            pulse_on_beat: true,
            color_cycle: true,
            color_cycle_speed: 0.1,
            
            base_color: Color32::from_rgb(100, 200, 255),
            accent_color: Color32::from_rgb(255, 100, 200),
            background_color: Color32::from_rgb(10, 10, 20),
            glow_intensity: 0.5,
            particle_count: 50,
            
            sparkle_count_on_beat: 5,
            sparkle_spawn_mode: SparkleSpawnMode::RandomArea,
            sparkle_radius: 200.0,
            sparkle_polygon_sides: 5,
            sparkle_size_min: 2.0,
            sparkle_size_max: 8.0,
            sparkle_speed_min: 100.0,
            sparkle_speed_max: 300.0,
            sparkle_grid_dims: Vec2::new(50.0, 50.0),
            sparkle_min_dist: 30.0,
            
            sparkle_always_on: true,
            sparkle_spawn_rate: 20.0,
            sparkle_life_min: 0.5,
            sparkle_life_max: 1.5,
            sparkle_alpha_min: 0.5,
            sparkle_alpha_max: 1.0,
            
            blur_enabled: false,
            blur_samples: 5,
            blur_opacity: 0.3,
            blur_depth_reduction: 2,
            
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
            self.trees = VisualizerConfig::default().trees;
        }
    }

// Particle for beat effects
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ParticleShape {
    Circle,
    Square,
    Triangle,
    Star,
    Hexagon,
}

#[derive(Clone)]
pub(crate) struct Particle {
    pub(crate) pos: Vec2, // Offset from center
    pub(crate) vel: Vec2,
    pub(crate) life: f32,
    pub(crate) max_life: f32,
    pub(crate) size: f32,
    pub(crate) color: Color32,
    pub(crate) max_alpha: f32,
    pub(crate) shape: ParticleShape,
}

impl Particle {
    fn new(offset: Vec2, angle: f32, speed: f32, size: f32, color: Color32, life: f32, max_alpha: f32, shape: ParticleShape) -> Self {
        Self {
            pos: offset,
            vel: Vec2::new(angle.cos() * speed, angle.sin() * speed),
            life,
            max_life: life,
            size,
            color,
            max_alpha,
            shape,
        }
    }
    
    fn update(&mut self, dt: f32) {
        self.pos += self.vel * dt;
        self.vel *= 0.98; // Friction
        self.life -= dt / self.max_life;
    }
    
    fn is_alive(&self) -> bool {
        self.life > 0.0
    }
}

// Simple random function for WASM
fn rand_float() -> f32 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    
    thread_local! {
        static SEED: RefCell<u64> = RefCell::new(12345);
    }
    
    SEED.with(|seed| {
        let mut s = seed.borrow_mut();
        let mut hasher = DefaultHasher::new();
        s.hash(&mut hasher);
        *s = hasher.finish();
        *s as f32 / u64::MAX as f32
    })
}

// Web audio types and initialization moved to `src/audio.rs`.

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SparkleSpawnMode {
    Point,
    RandomArea,
    Grid,
    Circle,
    Polygon,
    TreeOrigins,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SettingsTab {
    Trees,
    Sparkles,
    Blur,
}

// Main visualizer app
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum VisualizerMode {
    Fractal,
    UnknownPleasures,
}

pub struct MusicVisualizerApp {
    audio: AudioAnalysis,
    config: VisualizerConfig,
    #[allow(dead_code)]
    web_audio: WebAudio,
    
    // Animation state
    time: f64,
    rotation: f32,
    particles: Vec<Particle>,
    history: VecDeque<FrameState>,
    last_screen_rect: Rect,
    
    // Audio data shared with JS callback
    audio_data: Rc<RefCell<(Vec<u8>, Vec<u8>)>>,
    audio_initialized: Rc<RefCell<bool>>,
    
    // Playlist state (shared for file input callback)
    playlist: PlaylistState,
    pending_tracks: Rc<RefCell<Vec<(String, String, String)>>>, // (name, type, url)
    audio_element: Rc<RefCell<Option<web_sys::HtmlAudioElement>>>,
    audio_context: Rc<RefCell<Option<web_sys::AudioContext>>>,
    analyser_node: Rc<RefCell<Option<web_sys::AnalyserNode>>>,
    file_audio_initialized: Rc<RefCell<bool>>,
    
    // UI state
    demo_mode: bool,
    show_spectrum: bool,
    show_waveform: bool,
    show_settings: bool,
    beat_flash: f32,
    selected_tree_index: usize,
    edit_all_trees: bool,
    settings_tab: SettingsTab,
    // Option: when switching back to Fractal, reset fractal params to defaults
    restore_fractal_on_back: bool,
    // Current visualizer mode
    visualizer_mode: VisualizerMode,
    // Unknown Pleasures visualizer instance
    unknown_visualizer: UnknownPleasuresVisualizer,
    // System audio mode
    system_audio_mode: Option<bool>,
    // YouTube URL input buffer
    youtube_url_input: String,
    // YouTube error message
    youtube_error: Rc<RefCell<Option<String>>>,
}

impl Default for MusicVisualizerApp {
    fn default() -> Self {
        Self {
            audio: AudioAnalysis::new(),
            config: VisualizerConfig::default(),
            web_audio: WebAudio::default(),
            time: 0.0,
            rotation: 0.0,
            particles: Vec::new(),
            history: VecDeque::new(),
            last_screen_rect: Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0)),
            audio_data: Rc::new(RefCell::new((vec![0u8; 256], vec![0u8; 256]))),
            audio_initialized: Rc::new(RefCell::new(false)),
            playlist: PlaylistState::default(),
            pending_tracks: Rc::new(RefCell::new(Vec::new())),
            audio_element: Rc::new(RefCell::new(None)),
            audio_context: Rc::new(RefCell::new(None)),
            analyser_node: Rc::new(RefCell::new(None)),
            file_audio_initialized: Rc::new(RefCell::new(false)),
            demo_mode: true,
            show_spectrum: true,
            show_waveform: true,
            show_settings: true,
            beat_flash: 0.0,
            selected_tree_index: 0,
            edit_all_trees: false,
            settings_tab: SettingsTab::Trees,
            restore_fractal_on_back: false,
            visualizer_mode: VisualizerMode::Fractal,
            unknown_visualizer: UnknownPleasuresVisualizer::new(),
            system_audio_mode: Some(false),
            youtube_url_input: String::new(),
            youtube_error: Rc::new(RefCell::new(None)),
        }
    }
}

impl MusicVisualizerApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self::default()
    }
    
    fn try_init_audio(&mut self) {
        if *self.audio_initialized.borrow() {
            return;
        }
        
        let audio_data = self.audio_data.clone();
        let audio_initialized = self.audio_initialized.clone();
        
        spawn_local(async move {
            match init_web_audio(audio_data.clone()).await {
                Ok(_) => {
                    *audio_initialized.borrow_mut() = true;
                    web_sys::console::log_1(&"Audio initialized successfully!".into());
                }
                Err(e) => {
                    web_sys::console::error_1(&format!("Audio init failed: {:?}", e).into());
                }
            }
        });
    }
    
    fn update_audio(&mut self, dt: f32) {
        // Check if playing from file
        let is_file_playing = self.playlist.is_playing && *self.file_audio_initialized.borrow();
        
        if is_file_playing {
            // Use audio data from file playback
            let data = self.audio_data.borrow();
            self.audio.update_from_fft(&data.0, &data.1);
        } else if self.demo_mode || !*self.audio_initialized.borrow() {
            self.audio.simulate_demo(self.time);
        } else {
            // Use microphone audio data
            let data = self.audio_data.borrow();
            self.audio.update_from_fft(&data.0, &data.1);
        }
        
        // Beat flash decay
        if self.audio.beat {
            self.beat_flash = 1.0;
        }
        self.beat_flash *= 0.9_f32.powf(dt * 60.0);
    }

    pub fn get_band_amplitude(&self, band_idx: usize, total_bands: usize) -> f32 {
        let freq_len = self.audio.frequency_data.len().max(1);
        let f0 = (band_idx as f32) / (total_bands as f32);
        let f1 = ((band_idx + 1) as f32) / (total_bands as f32);
        // Use the same exponent as Unknown Pleasures for consistency, or a default
        let exp = self.config.up_freq_curve_exponent.max(0.001);
        
        let idx0 = ((f0.powf(exp)) * (freq_len as f32)).floor() as usize;
        let idx1 = ((f1.powf(exp)) * (freq_len as f32)).floor() as usize;
        
        let start = idx0.min(freq_len - 1);
        let mut end = idx1.min(freq_len);
        if end <= start { end = (start + 1).min(freq_len); }
        
        let mut sum = 0.0;
        for i in start..end {
            if i < self.audio.frequency_data.len() {
                sum += self.audio.frequency_data[i] as f32;
            }
        }
        
        let count = (end - start).max(1) as f32;
        (sum / count) / 255.0
    }
    
    fn update_animation(&mut self, dt: f32) {
        self.time += dt as f64;
        
        // Rotation
        if self.config.auto_rotate {
            let beat_boost = if self.audio.beat { self.config.rotation_beat_mult } else { 0.0 };
            self.rotation += (self.config.rotation_speed + beat_boost) * dt;
        }
        
        // Spawn particles on beat (disabled for Unknown Pleasures mode)
        let mut spawn_count = 0;
        if self.audio.beat && self.config.pulse_on_beat && self.visualizer_mode != VisualizerMode::UnknownPleasures {
            spawn_count += self.config.sparkle_count_on_beat;
        }
        
        // Continuous sparkles
        if self.config.sparkle_always_on && self.visualizer_mode != VisualizerMode::UnknownPleasures {
            let spawn_chance = self.config.sparkle_spawn_rate * dt;
            spawn_count += spawn_chance.floor() as u32 + if rand_float() < (spawn_chance % 1.0) { 1 } else { 0 };
        }

        if spawn_count > 0 {
            // Use last_screen_rect center as base, but we store offsets in particles
            // So 'center' here is just a reference point for calculation, but we want to store offset from screen center.
            // Actually, let's store offset from screen center.
            // The screen center is self.last_screen_rect.center().
            
            let screen_size = self.last_screen_rect.size();
            
            let color = self.get_current_color();
            
            for i in 0..spawn_count {
                let mut offset = Vec2::ZERO; // Default to center
                let mut angle = rand_float() * std::f32::consts::TAU;
                let speed = self.config.sparkle_speed_min + rand_float() * (self.config.sparkle_speed_max - self.config.sparkle_speed_min);
                let size = self.config.sparkle_size_min + rand_float() * (self.config.sparkle_size_max - self.config.sparkle_size_min);
                let life = self.config.sparkle_life_min + rand_float() * (self.config.sparkle_life_max - self.config.sparkle_life_min);
                let max_alpha = self.config.sparkle_alpha_min + rand_float() * (self.config.sparkle_alpha_max - self.config.sparkle_alpha_min);
                
                // Random shape
                let shape_idx = (rand_float() * 5.0) as u32;
                let shape = match shape_idx {
                    0 => ParticleShape::Circle,
                    1 => ParticleShape::Square,
                    2 => ParticleShape::Triangle,
                    3 => ParticleShape::Star,
                    _ => ParticleShape::Hexagon,
                };
                
                match self.config.sparkle_spawn_mode {
                    SparkleSpawnMode::Point => {
                        // Just center (offset 0)
                    },
                    SparkleSpawnMode::RandomArea => {
                        // Random position in full screen area
                        offset = Vec2::new(
                            (rand_float() - 0.5) * screen_size.x,
                            (rand_float() - 0.5) * screen_size.y
                        );
                    },
                    SparkleSpawnMode::Grid => {
                        let grid_size = (spawn_count as f32).sqrt().ceil() as i32;
                        let row = (i as i32) / grid_size;
                        let col = (i as i32) % grid_size;
                        
                        let offset_x = (col as f32 - grid_size as f32 / 2.0) * self.config.sparkle_grid_dims.x;
                        let offset_y = (row as f32 - grid_size as f32 / 2.0) * self.config.sparkle_grid_dims.y;
                        
                        offset = Vec2::new(offset_x, offset_y);
                        angle = 0.0; 
                    },
                    SparkleSpawnMode::Circle => {
                        let theta = rand_float() * std::f32::consts::TAU;
                        let r = self.config.sparkle_radius;
                        offset = Vec2::new(theta.cos() * r, theta.sin() * r);
                        angle = theta; // Outward
                    },
                    SparkleSpawnMode::Polygon => {
                        let sides = self.config.sparkle_polygon_sides.max(3);
                        let side_idx = (rand_float() * sides as f32) as u32;
                        let angle_per_side = std::f32::consts::TAU / sides as f32;
                        let a1 = side_idx as f32 * angle_per_side;
                        let a2 = (side_idx + 1) as f32 * angle_per_side;
                        let t = rand_float(); // Position along edge
                        
                        let p1 = Vec2::new(a1.cos() * self.config.sparkle_radius, a1.sin() * self.config.sparkle_radius);
                        let p2 = Vec2::new(a2.cos() * self.config.sparkle_radius, a2.sin() * self.config.sparkle_radius);
                        
                        offset = p1 + (p2 - p1) * t;
                        angle = offset.angle();
                    },
                    SparkleSpawnMode::TreeOrigins => {
                        if !self.config.trees.is_empty() {
                            let tree_idx = (rand_float() * self.config.trees.len() as f32) as usize;
                            if let Some(tree) = self.config.trees.get(tree_idx) {
                                offset = tree.origin_offset;
                            }
                        }
                    }
                }

                // Uniformity check (Poisson-disk-like rejection sampling)
                if self.config.sparkle_min_dist > 0.0 {
                    let mut valid = true;
                    // Check against recent particles (optimization: only check last N)
                    let check_count = 50.min(self.particles.len());
                    for p in self.particles.iter().rev().take(check_count) {
                        if (p.pos - offset).length() < self.config.sparkle_min_dist {
                            valid = false;
                            break;
                        }
                    }
                    
                    if !valid {
                        // Try to find a valid position a few times
                        for _ in 0..5 {
                            // Perturb position slightly to try to find a gap
                            let perturb = Vec2::new(
                                (rand_float() - 0.5) * self.config.sparkle_min_dist * 2.0,
                                (rand_float() - 0.5) * self.config.sparkle_min_dist * 2.0
                            );
                            let new_offset = offset + perturb;
                            
                            let mut retry_valid = true;
                            for p in self.particles.iter().rev().take(check_count) {
                                if (p.pos - new_offset).length() < self.config.sparkle_min_dist {
                                    retry_valid = false;
                                    break;
                                }
                            }
                            
                            if retry_valid {
                                offset = new_offset;
                                valid = true;
                                break;
                            }
                        }
                        
                        // If still not valid, skip spawning this particle
                        if !valid {
                            continue;
                        }
                    }
                }

                self.particles.push(Particle::new(offset, angle, speed, size, color, life, max_alpha, shape));
            }
        }
        
        // Update particles
        for p in &mut self.particles {
            p.update(dt);
        }
        self.particles.retain(|p| p.is_alive());
        
        // Limit particle count (config.particle_count is u32 now)
        while self.particles.len() > (self.config.particle_count as usize) * 2 {
            self.particles.remove(0);
        }
        
        // Update history for blur
        if self.config.blur_enabled {
            self.history.push_front(FrameState {
                audio: self.audio.clone(),
                rotation: self.rotation,
                time: self.time,
            });
            
            while self.history.len() > self.config.blur_samples {
                self.history.pop_back();
            }
        } else {
            self.history.clear();
        }
    }
    
    // get_current_color is provided by the UI module (impl in src/ui.rs)
    
    // UI drawing implementations moved to `src/ui.rs`.
    
    fn draw_settings_panel(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading("🎵 Music Visualizer");
            ui.separator();

            // Visualizer mode selector (list-style)
            ui.collapsing("🎚️ Visualizer Mode", |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Mode:");
                        if ui.selectable_label(self.visualizer_mode == VisualizerMode::Fractal, "Fractal").clicked() {
                            let prev = self.visualizer_mode;
                            self.visualizer_mode = VisualizerMode::Fractal;
                            if prev == VisualizerMode::UnknownPleasures && self.restore_fractal_on_back {
                                self.config.reset_fractal_to_default();
                            }
                        }
                        if ui.selectable_label(self.visualizer_mode == VisualizerMode::UnknownPleasures, "Unknown Pleasures").clicked() {
                            self.visualizer_mode = VisualizerMode::UnknownPleasures;
                        }
                    });

                // If Unknown Pleasures is selected, show mode-specific params
                if self.visualizer_mode == VisualizerMode::UnknownPleasures {
                    ui.add_space(4.0);
                    ui.label("Unknown Pleasures Settings:");
                    ui.horizontal(|ui| {
                        ui.label("Line thickness:");
                        ui.add(egui::DragValue::new(&mut self.config.up_line_thickness).speed(0.1));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Perspective:");
                        ui.add(egui::DragValue::new(&mut self.config.up_perspective).speed(0.05));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Vertical scale:");
                        ui.add(egui::DragValue::new(&mut self.config.up_vertical_scale).speed(0.1));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Max lines:");
                        ui.add(egui::DragValue::new(&mut self.config.up_max_lines).speed(1.0));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Samples per line:");
                        ui.add(egui::DragValue::new(&mut self.config.up_samples).speed(1.0));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Freq curve exp:");
                        ui.add(egui::DragValue::new(&mut self.config.up_freq_curve_exponent).speed(0.1));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Smoothing:");
                        ui.add(egui::DragValue::new(&mut self.config.up_smoothing).speed(0.01));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Color mode:");
                        if ui.checkbox(&mut self.config.up_monochrome, "Monochrome").changed() {
                            // toggle
                        }
                    });
                    ui.add_space(4.0);
                    if ui.button("Apply 'Image' Preset").clicked() {
                        self.config = VisualizerConfig::preset_unknown_pleasures_image();
                    }
                }
            });
            
            // ===== PLAYLIST SECTION =====
            ui.collapsing("🎶 Playlist", |ui| {
                // Add music button
                ui.horizontal(|ui| {
                    if ui.button("➕ Add Music").clicked() {
                        self.trigger_file_input();
                    }
                    
                    // Shuffle button
                    let shuffle_text = if self.playlist.is_shuffled { "🔀 On" } else { "🔀 Off" };
                    if ui.button(shuffle_text).clicked() {
                        self.playlist.is_shuffled = !self.playlist.is_shuffled;
                        if self.playlist.is_shuffled {
                            self.playlist.shuffle_playlist();
                        }
                    }
                });
                
                ui.label("Supported: MP3, WAV, OGG, FLAC, AAC, M4A");
                ui.add_space(4.0);
                
                // URL input (YouTube or direct audio link)
                ui.horizontal(|ui| {
                    ui.label("🔗");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.youtube_url_input)
                            .hint_text("Paste YouTube or audio URL…")
                            .desired_width(ui.available_width() - 40.0),
                    );
                });
                ui.horizontal(|ui| {
                    if ui.button("▶ Play").clicked() && !self.youtube_url_input.trim().is_empty() {
                        let url = self.youtube_url_input.clone();
                        let is_youtube = url.contains("youtube.com") || url.contains("youtu.be");
                        let (name, file_type) = if is_youtube {
                            let name = if let Some(id) = extract_youtube_id(&url) {
                                format!("YouTube - {}", id)
                            } else {
                                "YouTube".to_string()
                            };
                            (name, "youtube".to_string())
                        } else {
                            // Direct audio URL
                            let name = url.split('/').last().unwrap_or("Audio").to_string();
                            let ext = url.split('.').last().unwrap_or("mp3").to_lowercase();
                            (name, ext)
                        };
                        
                        self.playlist.tracks.push(PlaylistTrack {
                            name,
                            duration: 0.0,
                            file_type,
                            url,
                        });
                        let idx = self.playlist.tracks.len().saturating_sub(1);
                        self.play_track(idx);
                        self.youtube_url_input.clear();
                    }
                    if ui.button("➕ Queue").clicked() && !self.youtube_url_input.trim().is_empty() {
                        let url = self.youtube_url_input.clone();
                        let is_youtube = url.contains("youtube.com") || url.contains("youtu.be");
                        let (name, file_type) = if is_youtube {
                            let name = if let Some(id) = extract_youtube_id(&url) {
                                format!("YouTube - {}", id)
                            } else {
                                "YouTube".to_string()
                            };
                            (name, "youtube".to_string())
                        } else {
                            let name = url.split('/').last().unwrap_or("Audio").to_string();
                            let ext = url.split('.').last().unwrap_or("mp3").to_lowercase();
                            (name, ext)
                        };
                        
                        self.playlist.tracks.push(PlaylistTrack {
                            name,
                            duration: 0.0,
                            file_type,
                            url,
                        });
                        self.youtube_url_input.clear();
                    }
                });
                ui.add_space(4.0);
                
                // Current track info and progress
                if let Some(track) = self.playlist.get_current_track().cloned() {
                    ui.group(|ui| {
                        ui.label(format!("🎵 {}", track.name));
                        ui.label(format!("Format: {}", track.file_type.to_uppercase()));
                        
                        // Progress / seek
                        let current_time = PlaylistState::format_time(self.playlist.current_time);
                        let total_time = PlaylistState::format_time(self.playlist.duration);
                        
                        ui.horizontal(|ui| {
                            ui.label(&current_time);
                            
                            // Interactive progress slider
                            let duration = self.playlist.duration.max(1.0); // avoid divide by zero
                            let mut time = self.playlist.current_time;
                            let slider_response = ui.add(
                                egui::Slider::new(&mut time, 0.0..=duration)
                                    .show_value(false)
                                    .trailing_fill(true)
                            );
                            if slider_response.changed() || slider_response.drag_stopped() {
                                self.seek_to(time);
                            }
                            
                            ui.label(&total_time);
                        });
                        
                        // Playback controls
                        ui.horizontal(|ui| {
                            // Previous
                            if ui.button("⏮").clicked() {
                                self.play_previous();
                            }
                            
                            // Play/Pause
                            let play_pause_icon = if self.playlist.is_playing { "⏸" } else { "▶" };
                            if ui.button(play_pause_icon).clicked() {
                                self.toggle_playback();
                            }
                            
                            // Next
                            if ui.button("⏭").clicked() {
                                self.play_next();
                            }
                            
                            // Stop
                            if ui.button("⏹").clicked() {
                                self.stop_playback();
                            }
                        });
                        
                        // Volume control
                        ui.horizontal(|ui| {
                            ui.label("🔊");
                            if ui.add(egui::Slider::new(&mut self.playlist.volume, 0.0..=1.0).show_value(false)).changed() {
                                self.update_volume();
                            }
                        });
                    });
                    
                    // Show YouTube info/error if any
                    if let Some(err) = self.youtube_error.borrow().as_ref() {
                        ui.colored_label(Color32::from_rgb(255, 100, 100), format!("⚠ {}", err));
                        ui.label("Try pasting a direct audio URL instead.");
                    }
                } else {
                    ui.colored_label(Color32::GRAY, "No track selected");
                }
                
                ui.add_space(8.0);
                
                // Track list
                if !self.playlist.tracks.is_empty() {
                    ui.label(format!("Tracks ({}):", self.playlist.tracks.len()));
                    
                    let mut track_to_play: Option<usize> = None;
                    let mut track_to_remove: Option<usize> = None;
                    
                    egui::ScrollArea::vertical()
                        .max_height(150.0)
                        .id_salt("playlist_scroll")
                        .show(ui, |ui| {
                            for (idx, track) in self.playlist.tracks.iter().enumerate() {
                                let is_current = self.playlist.current_index == Some(idx);
                                let bg_color = if is_current {
                                    Color32::from_rgba_unmultiplied(100, 200, 255, 30)
                                } else {
                                    Color32::TRANSPARENT
                                };
                                
                                egui::Frame::new()
                                    .fill(bg_color)
                                    .inner_margin(4.0)
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            // Track number/playing indicator
                                            if is_current && self.playlist.is_playing {
                                                ui.label("▶");
                                            } else {
                                                ui.label(format!("{}.", idx + 1));
                                            }
                                            
                                            // Track name (clickable)
                                            let track_label = egui::Label::new(&track.name).sense(egui::Sense::click());
                                            if ui.add(track_label).clicked() {
                                                track_to_play = Some(idx);
                                            }
                                            
                                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                // Remove button
                                                if ui.small_button("✕").clicked() {
                                                    track_to_remove = Some(idx);
                                                }
                                                
                                                // Duration
                                                ui.label(PlaylistState::format_time(track.duration));
                                            });
                                        });
                                    });
                            }
                        });
                    
                    // Handle track actions after iteration
                    if let Some(idx) = track_to_play {
                        self.play_track(idx);
                    }
                    if let Some(idx) = track_to_remove {
                        self.remove_track(idx);
                    }
                    
                    // Clear all button
                    ui.add_space(4.0);
                    if ui.button("🗑 Clear Playlist").clicked() {
                        self.clear_playlist();
                    }
                }
            });
            
            ui.separator();
            
            // Audio source
            ui.horizontal(|ui| {
                ui.label("Audio Source:");
                if ui.selectable_label(self.demo_mode, "Demo").clicked() {
                    self.demo_mode = true;
                }
                if ui.selectable_label(!self.demo_mode && !self.is_system_audio(), "Microphone").clicked() {
                    self.demo_mode = false;
                    self.set_system_audio(false);
                    self.try_init_audio();
                }
                if ui.selectable_label(self.is_system_audio(), "System Audio").clicked() {
                    self.demo_mode = false;
                    self.set_system_audio(true);
                    self.try_init_system_audio();
                }
            });
            
            if !self.demo_mode && !*self.audio_initialized.borrow() {
                ui.colored_label(Color32::YELLOW, "⏳ Initializing microphone...");
            }
            
            ui.separator();
            
            // Audio levels
            ui.collapsing("📊 Audio Levels", |ui| {
                ui.horizontal(|ui| {
                    ui.label("Bass:");
                    ui.add(egui::ProgressBar::new(self.audio.smooth_bass).show_percentage());
                });
                ui.horizontal(|ui| {
                    ui.label("Mid:");
                    ui.add(egui::ProgressBar::new(self.audio.smooth_mid).show_percentage());
                });
                ui.horizontal(|ui| {
                    ui.label("Treble:");
                    ui.add(egui::ProgressBar::new(self.audio.smooth_treble).show_percentage());
                });
                ui.horizontal(|ui| {
                    ui.label("Volume:");
                    ui.add(egui::ProgressBar::new(self.audio.smooth_volume).show_percentage());
                });
                if self.audio.beat {
                    ui.colored_label(Color32::from_rgb(255, 100, 100), "🥁 BEAT!");
                }
            });
            
            ui.separator();
            
            // Fractal settings (now accepts arbitrary numbers via DragValue)
            ui.collapsing("🌿 Fractal & Sparkle Settings", |ui| {
                if self.visualizer_mode == VisualizerMode::UnknownPleasures {
                    ui.label("Unknown Pleasures Visualizer Controls:");
                    ui.horizontal(|ui| {
                        ui.label("Zoom:");
                        ui.add(egui::DragValue::new(&mut self.config.up_zoom).speed(0.01));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Isometric rotate:");
                        ui.checkbox(&mut self.config.up_isometric_rotate, "Enable");
                        ui.label("Angle:");
                        ui.add(egui::DragValue::new(&mut self.config.up_rotation_deg).speed(1.0));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Line thickness:");
                        ui.add(egui::DragValue::new(&mut self.config.up_line_thickness).speed(0.1));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Line length:");
                        ui.add(egui::DragValue::new(&mut self.config.up_line_length).speed(0.05));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Perspective:");
                        ui.add(egui::DragValue::new(&mut self.config.up_perspective).speed(0.05));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Vertical scale:");
                        ui.add(egui::DragValue::new(&mut self.config.up_vertical_scale).speed(0.1));
                    });
                } else {
                    // Tab Selector
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut self.settings_tab, SettingsTab::Trees, "🌲 Trees");
                        ui.selectable_value(&mut self.settings_tab, SettingsTab::Sparkles, "✨ Sparkles");
                        ui.selectable_value(&mut self.settings_tab, SettingsTab::Blur, "💨 Blur");
                    });
                    ui.separator();

                    match self.settings_tab {
                        SettingsTab::Trees => {
                            // Tree Management
                            ui.horizontal(|ui| {
                                ui.label("Select Tree:");
                                let selected_name = self.config.trees.get(self.selected_tree_index).map(|t| t.name.clone()).unwrap_or_else(|| "None".to_string());
                                egui::ComboBox::from_id_salt("tree_select")
                                    .selected_text(selected_name)
                                    .show_ui(ui, |ui| {
                                        for (i, tree) in self.config.trees.iter().enumerate() {
                                            ui.selectable_value(&mut self.selected_tree_index, i, &tree.name);
                                        }
                                    });
                                
                                if ui.button("➕ Add").clicked() {
                                    let count = self.config.trees.len() + 1;
                                    let mut new_tree = if self.edit_all_trees && !self.config.trees.is_empty() {
                                        // Inherit from first tree if edit all is on
                                        let mut t = self.config.trees[0].clone();
                                        t.name = format!("Tree {}", count);
                                        t
                                    } else {
                                        let mut t = TreeConfig::default();
                                        t.name = format!("Tree {}", count);
                                        t
                                    };
                                    
                                    new_tree.frequency_band = Some((count - 1) % 16);
                                    self.config.trees.push(new_tree);
                                    
                                    // Recalculate angles
                                    let step = 360.0 / count as f32;
                                    for (i, tree) in self.config.trees.iter_mut().enumerate() {
                                        tree.angle_offset = i as f32 * step;
                                    }
                                    self.selected_tree_index = count - 1;
                                }
                                
                                if self.config.trees.len() > 1 {
                                     if ui.button("🗑").clicked() {
                                         self.config.trees.remove(self.selected_tree_index);
                                         if self.selected_tree_index >= self.config.trees.len() {
                                             self.selected_tree_index = self.config.trees.len().saturating_sub(1);
                                         }
                                         // Recalculate angles
                                         let count = self.config.trees.len();
                                         if count > 0 {
                                             let step = 360.0 / count as f32;
                                             for (i, tree) in self.config.trees.iter_mut().enumerate() {
                                                 tree.angle_offset = i as f32 * step;
                                             }
                                         }
                                     }
                                }
                                
                                ui.add_space(8.0);
                                ui.toggle_value(&mut self.edit_all_trees, "Edit All Trees");
                                
                                if ui.button("🌲 Generate Random Forest").clicked() {
                                    self.config.trees.clear();
                                    let count = 10 + (rand_float() * 10.0) as usize;
                                    for i in 0..count {
                                        let mut t = TreeConfig::default();
                                        t.name = format!("Tree {}", i + 1);
                                        t.origin_offset = Vec2::new(
                                            (rand_float() - 0.5) * 600.0,
                                            (rand_float() - 0.5) * 400.0
                                        );
                                        t.base_zoom = 0.05 + rand_float() * 0.05;
                                        t.angle_offset = rand_float() * 360.0;
                                        t.frequency_band = Some(i % 16);
                                        self.config.trees.push(t.clone());
                                        // Add a second tree at same point (as requested)
                                        let mut t2 = t.clone();
                                        t2.name = format!("Tree {} (Pair)", i + 1);
                                        t2.angle_offset += 180.0; // Mirror
                                        self.config.trees.push(t2);
                                    }
                                    self.selected_tree_index = 0;
                                }
                            });
                            
                            ui.separator();
                            
                            if !self.config.trees.is_empty() {
                                if self.selected_tree_index >= self.config.trees.len() {
                                    self.selected_tree_index = 0;
                                }
                                
                                // Capture current values from selected tree
                                let mut name = self.config.trees[self.selected_tree_index].name.clone();
                                let mut angle_offset = self.config.trees[self.selected_tree_index].angle_offset;
                                let mut frequency_band = self.config.trees[self.selected_tree_index].frequency_band;
                                let mut base_zoom = self.config.trees[self.selected_tree_index].base_zoom;
                                let mut base_width = self.config.trees[self.selected_tree_index].base_width;
                                let mut base_depth = self.config.trees[self.selected_tree_index].base_depth;
                                let mut base_brightness = self.config.trees[self.selected_tree_index].base_brightness;
                                let mut origin_offset = self.config.trees[self.selected_tree_index].origin_offset;
                                let mut pseudo_3d = self.config.trees[self.selected_tree_index].pseudo_3d;
                                let mut tilt_x = self.config.trees[self.selected_tree_index].tilt_x;
                                let mut tilt_y = self.config.trees[self.selected_tree_index].tilt_y;

                                if !self.edit_all_trees {
                                    ui.horizontal(|ui| {
                                        ui.label("Name:");
                                        if ui.text_edit_singleline(&mut name).changed() {
                                            self.config.trees[self.selected_tree_index].name = name;
                                        }
                                    });
                                } else {
                                    ui.label("Editing ALL trees (Name editing disabled)");
                                }
                                
                                ui.collapsing("Position & 3D", |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label("Origin X:");
                                        if ui.add(egui::DragValue::new(&mut origin_offset.x).speed(1.0)).changed() {
                                            if self.edit_all_trees {
                                                for t in &mut self.config.trees { t.origin_offset.x = origin_offset.x; }
                                            } else {
                                                self.config.trees[self.selected_tree_index].origin_offset.x = origin_offset.x;
                                            }
                                        }
                                        ui.label("Y:");
                                        if ui.add(egui::DragValue::new(&mut origin_offset.y).speed(1.0)).changed() {
                                            if self.edit_all_trees {
                                                for t in &mut self.config.trees { t.origin_offset.y = origin_offset.y; }
                                            } else {
                                                self.config.trees[self.selected_tree_index].origin_offset.y = origin_offset.y;
                                            }
                                        }
                                    });
                                    
                                    if ui.checkbox(&mut pseudo_3d, "Pseudo 3D Effect").changed() {
                                        if self.edit_all_trees {
                                            for t in &mut self.config.trees { t.pseudo_3d = pseudo_3d; }
                                        } else {
                                            self.config.trees[self.selected_tree_index].pseudo_3d = pseudo_3d;
                                        }
                                    }
                                    
                                    if pseudo_3d {
                                        ui.horizontal(|ui| {
                                            ui.label("Tilt X:");
                                            if ui.add(egui::DragValue::new(&mut tilt_x).speed(1.0)).changed() {
                                                if self.edit_all_trees {
                                                    for t in &mut self.config.trees { t.tilt_x = tilt_x; }
                                                } else {
                                                    self.config.trees[self.selected_tree_index].tilt_x = tilt_x;
                                                }
                                            }
                                            ui.label("Tilt Y:");
                                            if ui.add(egui::DragValue::new(&mut tilt_y).speed(1.0)).changed() {
                                                if self.edit_all_trees {
                                                    for t in &mut self.config.trees { t.tilt_y = tilt_y; }
                                                } else {
                                                    self.config.trees[self.selected_tree_index].tilt_y = tilt_y;
                                                }
                                            }
                                        });
                                    }
                                });
                                
                                ui.horizontal(|ui| {
                                    ui.label("Angle Offset:");
                                    if ui.add(egui::DragValue::new(&mut angle_offset).speed(1.0)).changed() {
                                        if self.edit_all_trees {
                                            for t in &mut self.config.trees { t.angle_offset = angle_offset; }
                                        } else {
                                            self.config.trees[self.selected_tree_index].angle_offset = angle_offset;
                                        }
                                    }
                                });
                                
                                ui.horizontal(|ui| {
                                    ui.label("Freq Band (0-15):");
                                    let mut use_band = frequency_band.is_some();
                                    if ui.checkbox(&mut use_band, "Use Band").changed() {
                                        let new_val = if use_band { Some(0) } else { None };
                                        if self.edit_all_trees {
                                            for t in &mut self.config.trees { t.frequency_band = new_val; }
                                        } else {
                                            self.config.trees[self.selected_tree_index].frequency_band = new_val;
                                        }
                                        frequency_band = new_val;
                                    }
                                    
                                    if let Some(band) = frequency_band {
                                        let mut b = band as i32;
                                        if ui.add(egui::DragValue::new(&mut b).range(0..=15)).changed() {
                                            let new_band = Some(b as usize);
                                            if self.edit_all_trees {
                                                for t in &mut self.config.trees { t.frequency_band = new_band; }
                                            } else {
                                                self.config.trees[self.selected_tree_index].frequency_band = new_band;
                                            }
                                        }
                                    }
                                });

                                ui.horizontal(|ui| {
                                    ui.label("Zoom:");
                                    if ui.add(egui::DragValue::new(&mut base_zoom).speed(0.01)).changed() {
                                        if self.edit_all_trees {
                                            for t in &mut self.config.trees { t.base_zoom = base_zoom; }
                                        } else {
                                            self.config.trees[self.selected_tree_index].base_zoom = base_zoom;
                                        }
                                    }
                                });
                                ui.horizontal(|ui| {
                                    ui.label("Width:");
                                    if ui.add(egui::DragValue::new(&mut base_width).speed(0.01)).changed() {
                                        if self.edit_all_trees {
                                            for t in &mut self.config.trees { t.base_width = base_width; }
                                        } else {
                                            self.config.trees[self.selected_tree_index].base_width = base_width;
                                        }
                                    }
                                });
                                ui.horizontal(|ui| {
                                    ui.label("Depth:");
                                    if ui.add(egui::DragValue::new(&mut base_depth).speed(1.0)).changed() {
                                        if self.edit_all_trees {
                                            for t in &mut self.config.trees { t.base_depth = base_depth; }
                                        } else {
                                            self.config.trees[self.selected_tree_index].base_depth = base_depth;
                                        }
                                    }
                                });
                                ui.horizontal(|ui| {
                                    ui.label("Brightness:");
                                    if ui.add(egui::DragValue::new(&mut base_brightness).speed(0.01)).changed() {
                                        if self.edit_all_trees {
                                            for t in &mut self.config.trees { t.base_brightness = base_brightness; }
                                        } else {
                                            self.config.trees[self.selected_tree_index].base_brightness = base_brightness;
                                        }
                                    }
                                });
                            }
                        }
                        SettingsTab::Sparkles => {
                            ui.label("Sparkle Configuration");
                            ui.horizontal(|ui| {
                                ui.label("Count on Beat:");
                                ui.add(egui::DragValue::new(&mut self.config.sparkle_count_on_beat).speed(1.0).range(0..=500));
                            });
                            
                            ui.horizontal(|ui| {
                                ui.label("Spawn Mode:");
                                egui::ComboBox::from_id_salt("spawn_mode")
                                    .selected_text(match self.config.sparkle_spawn_mode {
                                        SparkleSpawnMode::Point => "Point",
                                        SparkleSpawnMode::RandomArea => "Random Area",
                                        SparkleSpawnMode::Grid => "Grid",
                                        SparkleSpawnMode::Circle => "Circle",
                                        SparkleSpawnMode::Polygon => "Polygon",
                                        SparkleSpawnMode::TreeOrigins => "Tree Origins",
                                    })
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(&mut self.config.sparkle_spawn_mode, SparkleSpawnMode::Point, "Point");
                                        ui.selectable_value(&mut self.config.sparkle_spawn_mode, SparkleSpawnMode::RandomArea, "Random Area");
                                        ui.selectable_value(&mut self.config.sparkle_spawn_mode, SparkleSpawnMode::Grid, "Grid");
                                        ui.selectable_value(&mut self.config.sparkle_spawn_mode, SparkleSpawnMode::Circle, "Circle");
                                        ui.selectable_value(&mut self.config.sparkle_spawn_mode, SparkleSpawnMode::Polygon, "Polygon");
                                        ui.selectable_value(&mut self.config.sparkle_spawn_mode, SparkleSpawnMode::TreeOrigins, "Tree Origins");
                                    });
                            });
                            
                            match self.config.sparkle_spawn_mode {
                                SparkleSpawnMode::Grid => {
                                    ui.horizontal(|ui| {
                                        ui.label("Grid Spacing X:");
                                        ui.add(egui::DragValue::new(&mut self.config.sparkle_grid_dims.x).speed(1.0));
                                    });
                                    ui.horizontal(|ui| {
                                        ui.label("Grid Spacing Y:");
                                        ui.add(egui::DragValue::new(&mut self.config.sparkle_grid_dims.y).speed(1.0));
                                    });
                                }
                                SparkleSpawnMode::Circle | SparkleSpawnMode::Polygon => {
                                    ui.horizontal(|ui| {
                                        ui.label("Radius:");
                                        ui.add(egui::DragValue::new(&mut self.config.sparkle_radius).speed(1.0));
                                    });
                                    if self.config.sparkle_spawn_mode == SparkleSpawnMode::Polygon {
                                        ui.horizontal(|ui| {
                                            ui.label("Sides:");
                                            ui.add(egui::DragValue::new(&mut self.config.sparkle_polygon_sides).range(3..=20).speed(1.0));
                                        });
                                    }
                                }
                                _ => {}
                            }
                            
                            ui.horizontal(|ui| {
                                ui.label("Min Distance (Uniformity):");
                                ui.add(egui::DragValue::new(&mut self.config.sparkle_min_dist).speed(1.0).range(0.0..=100.0));
                            });
                            
                            ui.separator();
                            ui.label("Particle Properties");
                            ui.horizontal(|ui| {
                                ui.label("Size Min:");
                                ui.add(egui::DragValue::new(&mut self.config.sparkle_size_min).speed(0.1));
                                ui.label("Max:");
                                ui.add(egui::DragValue::new(&mut self.config.sparkle_size_max).speed(0.1));
                            });
                            ui.horizontal(|ui| {
                                ui.label("Speed Min:");
                                ui.add(egui::DragValue::new(&mut self.config.sparkle_speed_min).speed(1.0));
                                ui.label("Max:");
                                ui.add(egui::DragValue::new(&mut self.config.sparkle_speed_max).speed(1.0));
                            });
                            
                            ui.separator();
                            ui.label("Continuous Sparkles");
                            ui.checkbox(&mut self.config.sparkle_always_on, "Always On");
                            if self.config.sparkle_always_on {
                                ui.horizontal(|ui| {
                                    ui.label("Spawn Rate (per sec):");
                                    ui.add(egui::DragValue::new(&mut self.config.sparkle_spawn_rate).speed(1.0));
                                });
                                ui.horizontal(|ui| {
                                    ui.label("Life Min:");
                                    ui.add(egui::DragValue::new(&mut self.config.sparkle_life_min).speed(0.1));
                                    ui.label("Max:");
                                    ui.add(egui::DragValue::new(&mut self.config.sparkle_life_max).speed(0.1));
                                });
                                ui.horizontal(|ui| {
                                    ui.label("Alpha Min:");
                                    ui.add(egui::DragValue::new(&mut self.config.sparkle_alpha_min).speed(0.01).range(0.0..=1.0));
                                    ui.label("Max:");
                                    ui.add(egui::DragValue::new(&mut self.config.sparkle_alpha_max).speed(0.01).range(0.0..=1.0));
                                });
                            }
                            
                            if ui.button("🎲 Randomize Sparkles").clicked() {
                                self.config.sparkle_count_on_beat = (rand_float() * 50.0) as u32;
                                self.config.sparkle_radius = 50.0 + rand_float() * 300.0;
                                self.config.sparkle_size_min = 1.0 + rand_float() * 5.0;
                                self.config.sparkle_size_max = self.config.sparkle_size_min + rand_float() * 10.0;
                                self.config.sparkle_speed_min = 50.0 + rand_float() * 200.0;
                                self.config.sparkle_speed_max = self.config.sparkle_speed_min + rand_float() * 300.0;
                                
                                // Randomize continuous params
                                self.config.sparkle_spawn_rate = 5.0 + rand_float() * 50.0;
                                self.config.sparkle_life_min = 0.2 + rand_float() * 1.0;
                                self.config.sparkle_life_max = self.config.sparkle_life_min + rand_float() * 2.0;
                                self.config.sparkle_alpha_min = 0.1 + rand_float() * 0.5;
                                self.config.sparkle_alpha_max = self.config.sparkle_alpha_min + rand_float() * (1.0 - self.config.sparkle_alpha_min);
                                
                                // Random mode
                                let mode_idx = (rand_float() * 5.0) as u32;
                                self.config.sparkle_spawn_mode = match mode_idx {
                                    0 => SparkleSpawnMode::Point,
                                    1 => SparkleSpawnMode::RandomArea,
                                    2 => SparkleSpawnMode::Grid,
                                    3 => SparkleSpawnMode::Circle,
                                    _ => SparkleSpawnMode::Polygon,
                                };
                            }
                        }
                        SettingsTab::Blur => {
                            ui.label("Motion Blur / Trails Configuration");
                            ui.checkbox(&mut self.config.blur_enabled, "Enable Blur");
                            
                            if self.config.blur_enabled {
                                ui.horizontal(|ui| {
                                    ui.label("Duration (Samples):");
                                    ui.add(egui::DragValue::new(&mut self.config.blur_samples).range(1..=20).speed(1.0));
                                });
                                
                                ui.horizontal(|ui| {
                                    ui.label("Intensity (Opacity):");
                                    ui.add(egui::DragValue::new(&mut self.config.blur_opacity).range(0.0..=1.0).speed(0.01));
                                });
                                
                                ui.horizontal(|ui| {
                                    ui.label("Depth Reduction:");
                                    ui.add(egui::DragValue::new(&mut self.config.blur_depth_reduction).range(0..=5).speed(1.0));
                                });
                            }
                        }
                    }

                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.restore_fractal_on_back, "Reset fractal to defaults when switching back");
                    });
                    
                    ui.add_space(4.0);
                    if ui.button("🎲 Randomize All Parameters").clicked() {
                        // Randomize Trees
                        for tree in &mut self.config.trees {
                            tree.base_zoom = 0.05 + rand_float() * 0.2;
                            tree.base_width = 0.5 + rand_float() * 1.5;
                            tree.base_depth = 10 + (rand_float() * 10.0) as u32;
                            tree.base_brightness = 0.5 + rand_float() * 0.5;
                            tree.zoom_bass_mult = rand_float() * 0.3;
                            tree.width_bass_mult = rand_float() * 0.5;
                            tree.depth_complexity_mult = rand_float() * 5.0;
                            tree.brightness_treble_mult = rand_float() * 0.8;
                        }
                        
                        // Randomize Sparkles
                        self.config.sparkle_count_on_beat = (rand_float() * 50.0) as u32;
                        self.config.sparkle_radius = 50.0 + rand_float() * 300.0;
                        self.config.sparkle_size_min = 1.0 + rand_float() * 5.0;
                        self.config.sparkle_size_max = self.config.sparkle_size_min + rand_float() * 10.0;
                        self.config.sparkle_speed_min = 50.0 + rand_float() * 200.0;
                        self.config.sparkle_speed_max = self.config.sparkle_speed_min + rand_float() * 300.0;
                        
                        // Randomize continuous params
                        self.config.sparkle_spawn_rate = 5.0 + rand_float() * 50.0;
                        self.config.sparkle_life_min = 0.2 + rand_float() * 1.0;
                        self.config.sparkle_life_max = self.config.sparkle_life_min + rand_float() * 2.0;
                        self.config.sparkle_alpha_min = 0.1 + rand_float() * 0.5;
                        self.config.sparkle_alpha_max = self.config.sparkle_alpha_min + rand_float() * (1.0 - self.config.sparkle_alpha_min);
                        
                        let mode_idx = (rand_float() * 5.0) as u32;
                        self.config.sparkle_spawn_mode = match mode_idx {
                            0 => SparkleSpawnMode::Point,
                            1 => SparkleSpawnMode::RandomArea,
                            2 => SparkleSpawnMode::Grid,
                            3 => SparkleSpawnMode::Circle,
                            _ => SparkleSpawnMode::Polygon,
                        };
                        self.config.sparkle_grid_dims.x = 20.0 + rand_float() * 80.0;
                        self.config.sparkle_grid_dims.y = 20.0 + rand_float() * 80.0;
                        
                        // Randomize Global
                        self.config.rotation_speed = (rand_float() - 0.5) * 2.0;
                        self.config.color_cycle_speed = rand_float() * 0.5;
                    }
                }
            });
            
            // Audio reactivity (accept arbitrary multipliers)
            ui.collapsing("🎛️ Audio Reactivity", |ui| {
                if self.visualizer_mode == VisualizerMode::UnknownPleasures {
                    ui.horizontal(|ui| {
                        ui.label("Bass influence:");
                        ui.add(egui::DragValue::new(&mut self.config.up_bass_mult).speed(0.01));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Mid influence:");
                        ui.add(egui::DragValue::new(&mut self.config.up_mid_mult).speed(0.01));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Treble influence:");
                        ui.add(egui::DragValue::new(&mut self.config.up_treble_mult).speed(0.01));
                    });
                } else {
                    if !self.config.trees.is_empty() {
                        if self.selected_tree_index >= self.config.trees.len() {
                            self.selected_tree_index = 0;
                        }
                        
                        let mut zoom_bass_mult = self.config.trees[self.selected_tree_index].zoom_bass_mult;
                        let mut width_bass_mult = self.config.trees[self.selected_tree_index].width_bass_mult;
                        let mut depth_complexity_mult = self.config.trees[self.selected_tree_index].depth_complexity_mult;
                        let mut brightness_treble_mult = self.config.trees[self.selected_tree_index].brightness_treble_mult;
                        
                        if self.edit_all_trees {
                            ui.label("Editing Reactivity for ALL trees");
                        } else {
                            ui.label(format!("Reactivity for: {}", self.config.trees[self.selected_tree_index].name));
                        }

                        ui.horizontal(|ui| {
                            ui.label("Bass → Zoom:");
                            if ui.add(egui::DragValue::new(&mut zoom_bass_mult).speed(0.01)).changed() {
                                if self.edit_all_trees {
                                    for t in &mut self.config.trees { t.zoom_bass_mult = zoom_bass_mult; }
                                } else {
                                    self.config.trees[self.selected_tree_index].zoom_bass_mult = zoom_bass_mult;
                                }
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.label("Bass → Width:");
                            if ui.add(egui::DragValue::new(&mut width_bass_mult).speed(0.01)).changed() {
                                if self.edit_all_trees {
                                    for t in &mut self.config.trees { t.width_bass_mult = width_bass_mult; }
                                } else {
                                    self.config.trees[self.selected_tree_index].width_bass_mult = width_bass_mult;
                                }
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.label("Complexity → Depth:");
                            if ui.add(egui::DragValue::new(&mut depth_complexity_mult).speed(0.1)).changed() {
                                if self.edit_all_trees {
                                    for t in &mut self.config.trees { t.depth_complexity_mult = depth_complexity_mult; }
                                } else {
                                    self.config.trees[self.selected_tree_index].depth_complexity_mult = depth_complexity_mult;
                                }
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.label("Treble → Brightness:");
                            if ui.add(egui::DragValue::new(&mut brightness_treble_mult).speed(0.01)).changed() {
                                if self.edit_all_trees {
                                    for t in &mut self.config.trees { t.brightness_treble_mult = brightness_treble_mult; }
                                } else {
                                    self.config.trees[self.selected_tree_index].brightness_treble_mult = brightness_treble_mult;
                                }
                            }
                        });
                    }
                }
            });
            
            // Animation (now accepts arbitrary speeds/values)
            ui.collapsing("✨ Animation", |ui| {
                ui.checkbox(&mut self.config.auto_rotate, "Auto Rotate");
                ui.horizontal(|ui| {
                    ui.label("Rotation Speed:");
                    ui.add(egui::DragValue::new(&mut self.config.rotation_speed).speed(0.01));
                });
                ui.checkbox(&mut self.config.pulse_on_beat, "Pulse on Beat");
                ui.checkbox(&mut self.config.color_cycle, "Color Cycle");
                ui.horizontal(|ui| {
                    ui.label("Color Speed:");
                    ui.add(egui::DragValue::new(&mut self.config.color_cycle_speed).speed(0.01));
                });
            });
            
            // Display options
            ui.collapsing("🖥️ Display", |ui| {
                ui.checkbox(&mut self.show_spectrum, "Show Spectrum");
                ui.checkbox(&mut self.show_waveform, "Show Waveform");
                ui.horizontal(|ui| {
                    ui.label("Glow:");
                    ui.add(egui::DragValue::new(&mut self.config.glow_intensity).speed(0.01));
                });
            });
            
            ui.separator();
            
            // Reset button
            if ui.button("🔄 Reset Settings").clicked() {
                self.config = VisualizerConfig::default();
            }
        });
    }
    
    // Helper methods for system audio
    fn is_system_audio(&self) -> bool {
        self.system_audio_mode.unwrap_or(false)
    }

    fn set_system_audio(&mut self, enabled: bool) {
        self.system_audio_mode = Some(enabled);
    }

    fn try_init_system_audio(&mut self) {
        // Placeholder: actual implementation depends on browser support
        web_sys::console::log_1(&"System audio capture requested (not implemented)".into());
    }
    
    // ===== PLAYLIST METHODS =====
    
    fn process_pending_tracks(&mut self) {
        // Process any tracks added from file input
        let pending = self.pending_tracks.borrow().clone();
        if !pending.is_empty() {
            for (name, file_type, url) in pending {
                self.playlist.tracks.push(PlaylistTrack {
                    name,
                    duration: 0.0, // Will be updated when metadata loads
                    file_type,
                    url,
                });
            }
            self.pending_tracks.borrow_mut().clear();
            
            // Auto-play first track if nothing is playing
            if self.playlist.current_index.is_none() && !self.playlist.tracks.is_empty() {
                self.playlist.current_index = Some(0);
            }
        }
    }
    
    fn trigger_file_input(&self) {
        // Create a hidden file input element and trigger it
        if let Some(window) = web_sys::window() {
            if let Some(document) = window.document() {
                // Check if input already exists
                let input = document
                    .get_element_by_id("music_file_input")
                    .and_then(|el| el.dyn_into::<web_sys::HtmlInputElement>().ok())
                    .unwrap_or_else(|| {
                        let input = document
                            .create_element("input")
                            .unwrap()
                            .dyn_into::<web_sys::HtmlInputElement>()
                            .unwrap();
                        input.set_type("file");
                        input.set_id("music_file_input");
                        input.set_accept("audio/*,.mp3,.wav,.ogg,.flac,.aac,.m4a");
                        input.set_multiple(true);
                        input.style().set_property("display", "none").ok();
                        document.body().unwrap().append_child(&input).ok();
                        input
                    });
                
                // Set up the change handler
                let audio_element = self.audio_element.clone();
                let audio_context = self.audio_context.clone();
                let analyser_node = self.analyser_node.clone();
                let audio_data = self.audio_data.clone();
                let file_audio_initialized = self.file_audio_initialized.clone();
                let pending_tracks = self.pending_tracks.clone();
                
                let closure = Closure::wrap(Box::new(move |event: web_sys::Event| {
                    let input = event.target()
                        .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok());
                    
                    if let Some(input) = input {
                        if let Some(files) = input.files() {
                            let window = web_sys::window().unwrap();
                            let document = window.document().unwrap();
                            
                            for i in 0..files.length() {
                                if let Some(file) = files.get(i) {
                                    let file_name = file.name();
                                    let file_type = file_name.split('.').last()
                                        .unwrap_or("unknown").to_lowercase();
                                    
                                    // Create object URL for the file
                                    if let Ok(url) = web_sys::Url::create_object_url_with_blob(&file) {
                                        // Add to pending tracks
                                        pending_tracks.borrow_mut().push((
                                            file_name.clone(),
                                            file_type.clone(),
                                            url.clone(),
                                        ));
                                        
                                        // Create or get audio element (only for first file or if none exists)
                                        if audio_element.borrow().is_none() || i == 0 {
                                            let audio = document
                                                .get_element_by_id("visualizer_audio")
                                                .and_then(|el| el.dyn_into::<web_sys::HtmlAudioElement>().ok())
                                                .unwrap_or_else(|| {
                                                    let audio = document
                                                        .create_element("audio")
                                                        .unwrap()
                                                        .dyn_into::<web_sys::HtmlAudioElement>()
                                                        .unwrap();
                                                    audio.set_id("visualizer_audio");
                                                    document.body().unwrap().append_child(&audio).ok();
                                                    audio
                                                });
                                            
                                            audio.set_src(&url);
                                            *audio_element.borrow_mut() = Some(audio.clone());
                                            
                                            // Initialize audio context if needed
                                            if audio_context.borrow().is_none() {
                                                if let Ok(ctx) = web_sys::AudioContext::new() {
                                                    if let Ok(analyser) = ctx.create_analyser() {
                                                        analyser.set_fft_size(512);
                                                        analyser.set_smoothing_time_constant(0.8);
                                                        
                                                        if let Ok(source) = ctx.create_media_element_source(&audio) {
                                                            source.connect_with_audio_node(&analyser).ok();
                                                            analyser.connect_with_audio_node(&ctx.destination()).ok();
                                                            
                                                            *analyser_node.borrow_mut() = Some(analyser);
                                                            *audio_context.borrow_mut() = Some(ctx);
                                                            *file_audio_initialized.borrow_mut() = true;
                                                            
                                                            // Start polling audio data
                                                            let analyser_for_poll = analyser_node.clone();
                                                            let audio_data_for_poll = audio_data.clone();
                                                            
                                                            let poll_callback = Closure::wrap(Box::new(move || {
                                                                if let Some(ref analyser) = *analyser_for_poll.borrow() {
                                                                    let freq_len = analyser.frequency_bin_count() as usize;
                                                                    let time_len = analyser.fft_size() as usize;
                                                                    let mut freq_data = vec![0u8; freq_len];
                                                                    let mut time_data = vec![0u8; time_len];
                                                                    
                                                                    analyser.get_byte_frequency_data(&mut freq_data);
                                                                    analyser.get_byte_time_domain_data(&mut time_data);
                                                                    
                                                                    *audio_data_for_poll.borrow_mut() = (freq_data, time_data);
                                                                }
                                                            }) as Box<dyn Fn()>);
                                                            
                                                            window.set_interval_with_callback_and_timeout_and_arguments_0(
                                                                poll_callback.as_ref().unchecked_ref(),
                                                                16,
                                                            ).ok();
                                                            poll_callback.forget();
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        
                                        web_sys::console::log_1(&format!("Added track: {} ({})", file_name, file_type).into());
                                    }
                                }
                            }
                        }
                        input.set_value(""); // Reset for next use
                    }
                }) as Box<dyn FnMut(web_sys::Event)>);
                
                input.set_onchange(Some(closure.as_ref().unchecked_ref()));
                closure.forget();
                
                input.click();
            }
        }
    }
    
    fn play_track(&mut self, index: usize) {
        if index >= self.playlist.tracks.len() {
            return;
        }

        self.playlist.current_index = Some(index);
        self.playlist.is_playing = true;

        // Get the URL from the playlist entry
        let track = self.playlist.tracks[index].clone();

        if track.file_type == "youtube" {
            // YouTube: show embedded player, use demo mode for visualization
            self.demo_mode = true; // Visualization will use demo/simulated audio
            *self.youtube_error.borrow_mut() = None;
            
            if let Some(video_id) = extract_youtube_id(&track.url) {
                // Create or update YouTube iframe player
                if let Some(window) = web_sys::window() {
                    if let Some(document) = window.document() {
                        // Pause any existing audio element
                        if let Some(ref audio) = *self.audio_element.borrow() {
                            audio.pause().ok();
                        }

                        // Create YouTube player container if it doesn't exist
                        let container = document
                            .get_element_by_id("youtube_player_container")
                            .unwrap_or_else(|| {
                                let div = document.create_element("div").unwrap();
                                div.set_id("youtube_player_container");
                                div.set_attribute("style", 
                                    "position:fixed; bottom:10px; right:10px; z-index:9999; \
                                     background:#222; border-radius:8px; padding:5px; \
                                     box-shadow: 0 4px 12px rgba(0,0,0,0.5);"
                                ).ok();
                                document.body().unwrap().append_child(&div).ok();
                                div
                            });

                        // Set iframe content
                        let embed_url = format!(
                            "https://www.youtube.com/embed/{}?autoplay=1&controls=1",
                            video_id
                        );
                        container.set_inner_html(&format!(
                            r#"<iframe width="320" height="180" src="{}" 
                               frameborder="0" allow="autoplay; encrypted-media" 
                               allowfullscreen style="border-radius:4px;"></iframe>
                               <button onclick="this.parentElement.style.display='none'" 
                               style="position:absolute;top:-8px;right:-8px;background:#ff4444;
                               color:white;border:none;border-radius:50%;width:24px;height:24px;
                               cursor:pointer;font-size:14px;">✕</button>"#,
                            embed_url
                        ));
                    }
                }
            } else {
                *self.youtube_error.borrow_mut() = Some("Could not extract YouTube video ID".to_string());
            }
        } else {
            // Standard audio file playback
            self.demo_mode = false;
            self.play_audio_url(&track.url);
            
            // Hide YouTube player if visible
            if let Some(window) = web_sys::window() {
                if let Some(document) = window.document() {
                    if let Some(container) = document.get_element_by_id("youtube_player_container") {
                        container.set_attribute("style", "display:none;").ok();
                    }
                }
            }
        }
    }
    
    /// Play audio from a direct URL (used for both local files and resolved YouTube audio)
    fn play_audio_url(&mut self, url: &str) {
        // Initialize audio element and context if needed
        self.ensure_audio_element_initialized();
        
        if let Some(ref audio) = *self.audio_element.borrow() {
            audio.set_src(url);
            audio.set_current_time(0.0);
            let _ = audio.play();
        }
    }
    
    /// Ensure audio element and WebAudio context are initialized for analysis
    fn ensure_audio_element_initialized(&mut self) {
        if self.audio_element.borrow().is_some() {
            return;
        }
        
        if let Some(window) = web_sys::window() {
            if let Some(document) = window.document() {
                let audio = document
                    .get_element_by_id("visualizer_audio")
                    .and_then(|el| el.dyn_into::<web_sys::HtmlAudioElement>().ok())
                    .unwrap_or_else(|| {
                        let a = document
                            .create_element("audio")
                            .unwrap()
                            .dyn_into::<web_sys::HtmlAudioElement>()
                            .unwrap();
                        a.set_id("visualizer_audio");
                        a.set_cross_origin(Some("anonymous"));
                        document.body().unwrap().append_child(&a).ok();
                        a
                    });
                
                *self.audio_element.borrow_mut() = Some(audio.clone());
                
                // Initialize audio context for analysis
                if self.audio_context.borrow().is_none() {
                    if let Ok(ctx) = web_sys::AudioContext::new() {
                        if let Ok(analyser) = ctx.create_analyser() {
                            analyser.set_fft_size(512);
                            analyser.set_smoothing_time_constant(0.8);
                            
                            if let Ok(source) = ctx.create_media_element_source(&audio) {
                                source.connect_with_audio_node(&analyser).ok();
                                analyser.connect_with_audio_node(&ctx.destination()).ok();
                                
                                *self.analyser_node.borrow_mut() = Some(analyser);
                                *self.audio_context.borrow_mut() = Some(ctx);
                                *self.file_audio_initialized.borrow_mut() = true;
                                
                                // Start polling audio data
                                let analyser_for_poll = self.analyser_node.clone();
                                let audio_data_for_poll = self.audio_data.clone();
                                
                                let poll_callback = Closure::wrap(Box::new(move || {
                                    if let Some(ref analyser) = *analyser_for_poll.borrow() {
                                        let freq_len = analyser.frequency_bin_count() as usize;
                                        let time_len = analyser.fft_size() as usize;
                                        let mut freq_data = vec![0u8; freq_len];
                                        let mut time_data = vec![0u8; time_len];
                                        
                                        analyser.get_byte_frequency_data(&mut freq_data);
                                        analyser.get_byte_time_domain_data(&mut time_data);
                                        
                                        *audio_data_for_poll.borrow_mut() = (freq_data, time_data);
                                    }
                                }) as Box<dyn Fn()>);
                                
                                window.set_interval_with_callback_and_timeout_and_arguments_0(
                                    poll_callback.as_ref().unchecked_ref(),
                                    16,
                                ).ok();
                                poll_callback.forget();
                            }
                        }
                    }
                }
            }
        }
    }
    
    fn toggle_playback(&mut self) {
        if let Some(ref audio) = *self.audio_element.borrow() {
            if self.playlist.is_playing {
                let _ = audio.pause();
                self.playlist.is_playing = false;
            } else {
                let _ = audio.play();
                self.playlist.is_playing = true;
            }
        }
    }
    
    fn stop_playback(&mut self) {
        if let Some(ref audio) = *self.audio_element.borrow() {
            audio.pause().ok();
            audio.set_current_time(0.0);
        }
        self.playlist.is_playing = false;
        self.playlist.current_time = 0.0;
    }
    
    fn play_next(&mut self) {
        if let Some(next_idx) = self.playlist.get_next_index() {
            self.play_track(next_idx);
        } else if !self.playlist.tracks.is_empty() {
            // Loop to first track
            self.play_track(0);
        } else {
            self.stop_playback();
        }
    }
    
    fn play_previous(&mut self) {
        // Si han pasado más de 3 segundos, reinicia la canción actual
        if self.playlist.current_time > 3.0 {
            self.seek_to(0.0);
            return;
        }

        if let Some(prev_idx) = self.playlist.get_prev_index() {
            self.play_track(prev_idx);
        } else {
            // Si no hay anterior, detener reproducción
            self.stop_playback();
        }
    }
    
    fn seek_to(&mut self, time: f64) {
        if let Some(ref audio) = *self.audio_element.borrow() {
            let duration = audio.duration();
            let seek_time = if !duration.is_nan() { time.min(duration) } else { time };
            audio.set_current_time(seek_time);
            self.playlist.current_time = seek_time;
        }
    }
    
    fn update_volume(&self) {
        if let Some(ref audio) = *self.audio_element.borrow() {
            audio.set_volume(self.playlist.volume as f64);
        }
    }
    
    fn remove_track(&mut self, index: usize) {
        if index < self.playlist.tracks.len() {
            self.playlist.tracks.remove(index);
            
            // Update current index if needed
            if let Some(current) = self.playlist.current_index {
                if index == current {
                    self.stop_playback();
                    self.playlist.current_index = None;
                } else if index < current {
                    self.playlist.current_index = Some(current - 1);
                }
            }
            
            // Update shuffle order
            if self.playlist.is_shuffled {
                self.playlist.shuffle_playlist();
            }
        }
    }
    
    fn clear_playlist(&mut self) {
        self.stop_playback();
        self.playlist.tracks.clear();
        self.playlist.current_index = None;
        self.playlist.shuffle_order.clear();
    }
    
    fn update_playback_state(&mut self) {
        let (current_time, duration, ended) = {
            if let Some(ref audio) = *self.audio_element.borrow() {
                let ct = audio.current_time();
                let dur = audio.duration();
                let track_ended = !dur.is_nan() && ct >= dur - 0.1;
                (ct, dur, track_ended)
            } else {
                return;
            }
        };
        
        self.playlist.current_time = current_time;
        self.playlist.duration = duration;
        
        // Check if track ended
        if ended {
            self.play_next();
        }
    }
}

impl eframe::App for MusicVisualizerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let dt = ctx.input(|i| i.stable_dt);
        
        // Process any pending tracks from file input
        self.process_pending_tracks();
        
        // Update playback state from audio element
        self.update_playback_state();
        
        // Update audio and animation
        self.update_audio(dt);
        self.update_animation(dt);
        
        // Request repaint for animation
        ctx.request_repaint();
        
        // Side panel for settings
        if self.show_settings {
            egui::SidePanel::left("settings_panel")
                .resizable(true)
                .default_width(280.0)
                .show(ctx, |ui| {
                    self.draw_settings_panel(ui);
                });
        }
        
        // Main visualization area
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(self.config.background_color))
            .show(ctx, |ui| {
            let _available = ui.available_rect_before_wrap();
            
            // Toggle settings button
            ui.horizontal(|ui| {
                if ui.button(if self.show_settings { "◀ Hide" } else { "▶ Show" }).clicked() {
                    self.show_settings = !self.show_settings;
                }
                ui.label(format!("FPS: {:.0}", 1.0 / dt));
            });
            
            let remaining = ui.available_rect_before_wrap();
            
            // Layout visualization areas
            let spectrum_height = if self.show_spectrum { 80.0 } else { 0.0 };
            let waveform_height = if self.show_waveform { 60.0 } else { 0.0 };
            let bottom_ui_height = spectrum_height + waveform_height;
            
            let fractal_rect = Rect::from_min_max(
                remaining.min,
                Pos2::new(remaining.max.x, remaining.max.y - bottom_ui_height),
            );
            
            // Update last screen rect for particle spawning
            self.last_screen_rect = fractal_rect;
            
            // Draw appropriate visualizer for selected mode
            let fractal_response = ui.allocate_rect(fractal_rect, egui::Sense::hover());
            if fractal_response.hovered() {
                // Could add mouse interaction here
            }
            match self.visualizer_mode {
                VisualizerMode::Fractal => self.draw_fractal(ui, fractal_rect),
                VisualizerMode::UnknownPleasures => {
                    // Delegate drawing to the Unknown Pleasures visualizer (mutable)
                    self.unknown_visualizer.draw(ui, fractal_rect, &self.audio, &self.config, self.time);
                }
            }
            
            // Draw particles
            let painter = ui.painter();
            self.draw_particles(painter, fractal_rect.center());
            
            // Draw spectrum analyzer
            if self.show_spectrum {
                let spectrum_rect = Rect::from_min_max(
                    Pos2::new(remaining.min.x, remaining.max.y - bottom_ui_height),
                    Pos2::new(remaining.max.x, remaining.max.y - waveform_height),
                );
                ui.allocate_rect(spectrum_rect, egui::Sense::hover());
                self.draw_spectrum(ui, spectrum_rect);
            }
            
            // Draw waveform
            if self.show_waveform {
                let waveform_rect = Rect::from_min_max(
                    Pos2::new(remaining.min.x, remaining.max.y - waveform_height),
                    remaining.max,
                );
                ui.allocate_rect(waveform_rect, egui::Sense::hover());
                self.draw_waveform(ui, waveform_rect);
            }
        });
    }
}

// HSL helper moved to `src/ui.rs` (used by drawing helpers there).

// init_web_audio moved to src/audio.rs

/// Extract YouTube video ID from various URL formats (youtube.com/watch?v=..., youtu.be/..., etc.)
fn extract_youtube_id(url: &str) -> Option<String> {
    // Try youtu.be/VIDEO_ID
    if url.contains("youtu.be/") {
        let parts: Vec<&str> = url.split("youtu.be/").collect();
        if let Some(part) = parts.get(1) {
            let id = part.split('?').next().unwrap_or(part).split('&').next().unwrap_or(part);
            if !id.is_empty() {
                return Some(id.to_string());
            }
        }
    }
    // Try youtube.com/watch?v=VIDEO_ID
    if url.contains("v=") {
        let parts: Vec<&str> = url.split("v=").collect();
        if let Some(part) = parts.get(1) {
            let id = part.split('&').next().unwrap_or(part);
            if !id.is_empty() {
                return Some(id.to_string());
            }
        }
    }
    // Try youtube.com/embed/VIDEO_ID
    if url.contains("/embed/") {
        let parts: Vec<&str> = url.split("/embed/").collect();
        if let Some(part) = parts.get(1) {
            let id = part.split('?').next().unwrap_or(part).split('&').next().unwrap_or(part);
            if !id.is_empty() {
                return Some(id.to_string());
            }
        }
    }
    None
}

// WASM entry point
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    console_error_panic_hook::set_once();
    
    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window()
            .expect("no global window exists")
            .document()
            .expect("should have a document on window");
        
        let canvas = document
            .get_element_by_id("music_visualizer_canvas")
            .expect("no canvas element with id 'music_visualizer_canvas'")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("element with id 'music_visualizer_canvas' is not a canvas");
        
        let web_options = eframe::WebOptions::default();
        
        eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(|cc| Ok(Box::new(MusicVisualizerApp::new(cc)))),
            )
            .await
            .expect("Failed to start eframe");
    });
    
    Ok(())
}
