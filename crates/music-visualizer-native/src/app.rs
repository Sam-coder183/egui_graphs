use std::sync::{Arc, Mutex};
use eframe::egui::{self, Color32, Pos2, Rect};
use crate::audio_analysis::AudioAnalysis;
use crate::visualizer_config::VisualizerConfig;
use crate::playlist::PlaylistState;
use crate::particle::Particle;
use crate::unknown_pleasures::UnknownPleasuresVisualizer;
// `ui` helpers are accessed explicitly where needed; avoid glob import which was unused.
use rodio::{OutputStream, OutputStreamHandle, Sink, Decoder, Source};
use std::fs::File;
use std::io::BufReader;
use std::time::Instant;
use rfd::FileDialog;
use hound;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum VisualizerMode {
    Fractal,
    UnknownPleasures,
}

pub struct MusicVisualizerNativeApp {
    pub audio: AudioAnalysis,
    pub config: VisualizerConfig,
    pub time: f64,
    pub rotation: f32,
    pub particles: Vec<Particle>,
    pub playlist: PlaylistState,
    pub beat_flash: f32,
    pub visualizer_mode: VisualizerMode,
    pub unknown_visualizer: UnknownPleasuresVisualizer,
    pub audio_data: Arc<Mutex<Vec<f32>>>,
    // demo mode uses generated audio if true
    pub demo_mode: bool,
    pub show_settings: bool,
    pub show_spectrum: bool,
    pub show_waveform: bool,
    // system audio capture flag
    pub system_audio_mode: bool,
    // rodio output for file playback
    pub output_stream: Option<OutputStream>,
    pub output_stream_handle: Option<OutputStreamHandle>,
    pub current_sink: Option<Sink>,
    // playback timing helpers for approximate tracking / seeking
    pub playback_start_instant: Option<Instant>,
    pub playback_seek_offset: f64,
}

impl MusicVisualizerNativeApp {
    pub fn with_audio_data(audio_data: Arc<Mutex<Vec<f32>>>) -> Self {
        Self {
            audio: AudioAnalysis::new(),
            config: VisualizerConfig::default(),
            time: 0.0,
            rotation: 0.0,
            particles: Vec::new(),
            playlist: PlaylistState::default(),
            beat_flash: 0.0,
            visualizer_mode: VisualizerMode::Fractal,
            unknown_visualizer: UnknownPleasuresVisualizer::new(),
            audio_data,
            show_settings: false,
            show_spectrum: false,
            show_waveform: false,
            system_audio_mode: true,
            demo_mode: true,
            output_stream: None,
            output_stream_handle: None,
            current_sink: None,
            playback_start_instant: None,
            playback_seek_offset: 0.0,
        }
    }

    pub fn draw_settings_panel(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading("🎵 Music Visualizer Native");
            ui.separator();

            // Visualizer mode selector
            ui.collapsing("🎚️ Visualizer Mode", |ui| {
                ui.horizontal(|ui| {
                    ui.label("Mode:");
                    if ui.selectable_label(self.visualizer_mode == VisualizerMode::Fractal, "Fractal").clicked() {
                        self.visualizer_mode = VisualizerMode::Fractal;
                    }
                    if ui.selectable_label(self.visualizer_mode == VisualizerMode::UnknownPleasures, "Unknown Pleasures").clicked() {
                        self.visualizer_mode = VisualizerMode::UnknownPleasures;
                    }
                });

                // Unknown Pleasures quick preset
                if self.visualizer_mode == VisualizerMode::UnknownPleasures {
                    ui.add_space(4.0);
                    if ui.button("Apply 'Image' Preset").clicked() {
                        self.config = VisualizerConfig::preset_unknown_pleasures_image();
                    }
                }
            });

            // ===== PLAYLIST SECTION =====
            ui.collapsing("🎶 Playlist", |ui| {
                ui.horizontal(|ui| {
                    if ui.button("➕ Add Music").clicked() {
                        self.trigger_file_input();
                    }
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

                if let Some(track) = self.playlist.get_current_track().cloned() {
                    ui.group(|ui| {
                        ui.label(format!("🎵 {}", track.name));
                        ui.label(format!("Format: {}", track.file_type.to_uppercase()));

                        // Progress bar with seeking
                        let progress = self.playlist.get_progress();
                        let current_time = PlaylistState::format_time(self.playlist.current_time);
                        let total_time = PlaylistState::format_time(self.playlist.duration);
                        ui.horizontal(|ui| {
                            ui.label(&current_time);
                            let progress_response = ui.add(
                                egui::ProgressBar::new(progress)
                                    .desired_width(ui.available_width() - 50.0)
                            );
                            if progress_response.clicked() || progress_response.dragged() {
                                if let Some(pos) = progress_response.interact_pointer_pos() {
                                    let rect = progress_response.rect;
                                    let seek_ratio = (pos.x - rect.left()) / rect.width();
                                    let seek_time = seek_ratio as f64 * self.playlist.duration;
                                    self.seek_to(seek_time);
                                }
                            }
                            ui.label(&total_time);
                        });

                        ui.horizontal(|ui| {
                            if ui.button("⏮").clicked() { self.play_previous(); }
                            let play_pause = if self.playlist.is_playing { "⏸" } else { "▶" };
                            if ui.button(play_pause).clicked() { self.toggle_playback(); }
                            if ui.button("⏭").clicked() { self.play_next(); }
                            if ui.button("⏹").clicked() { self.stop_playback(); }
                        });

                        ui.horizontal(|ui| {
                            ui.label("🔊");
                            if ui.add(egui::Slider::new(&mut self.playlist.volume, 0.0..=1.0).show_value(false)).changed() {
                                self.update_volume();
                            }
                        });
                    });
                } else {
                    ui.colored_label(Color32::GRAY, "No track selected");
                }

                ui.add_space(8.0);
                if !self.playlist.tracks.is_empty() {
                    ui.label(format!("Tracks ({}):", self.playlist.tracks.len()));
                    let mut play_idx: Option<usize> = None;
                    let mut remove_idx: Option<usize> = None;
                    egui::ScrollArea::vertical().max_height(150.0).show(ui, |ui| {
                        for (idx, track) in self.playlist.tracks.iter().enumerate() {
                            let is_current = self.playlist.current_index == Some(idx);
                            let bg = if is_current { Color32::from_rgba_unmultiplied(100,200,255,30) } else { Color32::TRANSPARENT };
                            egui::Frame::group(&ui.style()).fill(bg).inner_margin(4.0).show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    if is_current && self.playlist.is_playing { ui.label("▶"); } else { ui.label(format!("{}.", idx+1)); }
                                    let lbl = egui::Label::new(&track.name).sense(egui::Sense::click());
                                    if ui.add(lbl).clicked() { play_idx = Some(idx); }
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if ui.small_button("✕").clicked() { remove_idx = Some(idx); }
                                        ui.label(PlaylistState::format_time(track.duration));
                                    });
                                });
                            });
                        }

                    });
                    if let Some(i) = play_idx { self.play_track(i); }
                    if let Some(i) = remove_idx { self.remove_track(i); }
                    ui.add_space(4.0);
                    if ui.button("🗑 Clear Playlist").clicked() { self.clear_playlist(); }
                }
            });

            ui.separator();

            // Audio source selection
            ui.horizontal(|ui| {
                ui.label("Audio Source:");
                if ui.selectable_label(self.demo_mode, "Demo").clicked() { self.demo_mode = true; }
                if ui.selectable_label(!self.demo_mode && !self.is_system_audio(), "Microphone").clicked() {
                    self.demo_mode = false;
                    self.set_system_audio(false);
                }
                if ui.selectable_label(self.is_system_audio(), "System Audio").clicked() {
                    self.demo_mode = false;
                    self.set_system_audio(true);
                    self.try_init_system_audio();
                }
            });

            // Audio levels
            ui.collapsing("📊 Audio Levels", |ui| {
                ui.horizontal(|ui| { ui.label("Bass:"); ui.add(egui::ProgressBar::new(self.audio.smooth_bass).show_percentage()); });
                ui.horizontal(|ui| { ui.label("Mid:"); ui.add(egui::ProgressBar::new(self.audio.smooth_mid).show_percentage()); });
                ui.horizontal(|ui| { ui.label("Treble:"); ui.add(egui::ProgressBar::new(self.audio.smooth_treble).show_percentage()); });
                ui.horizontal(|ui| { ui.label("Volume:"); ui.add(egui::ProgressBar::new(self.audio.smooth_volume).show_percentage()); });
                if self.audio.beat { ui.colored_label(Color32::from_rgb(255,100,100), "🥁 BEAT!"); }
            });

            ui.separator();

            // Fractal / Unknown Pleasures settings and Reactivity
            ui.collapsing("🌿 Fractal Settings", |ui| {
                if self.visualizer_mode == VisualizerMode::UnknownPleasures {
                    ui.horizontal(|ui| { ui.label("Zoom:"); ui.add(egui::DragValue::new(&mut self.config.up_zoom).speed(0.01)); });
                    ui.horizontal(|ui| { ui.label("Isometric rotate:"); ui.checkbox(&mut self.config.up_isometric_rotate, "Enable"); ui.label("Angle:"); ui.add(egui::DragValue::new(&mut self.config.up_rotation_deg).speed(1.0)); });
                    ui.horizontal(|ui| { ui.label("Line thickness:"); ui.add(egui::DragValue::new(&mut self.config.up_line_thickness).speed(0.1)); });
                    ui.horizontal(|ui| { ui.label("Line length:"); ui.add(egui::DragValue::new(&mut self.config.up_line_length).speed(0.05)); });
                    ui.horizontal(|ui| { ui.label("Perspective:"); ui.add(egui::DragValue::new(&mut self.config.up_perspective).speed(0.05)); });
                    ui.horizontal(|ui| { ui.label("Vertical scale:"); ui.add(egui::DragValue::new(&mut self.config.up_vertical_scale).speed(0.1)); });
                } else {
                    ui.horizontal(|ui| { ui.label("Zoom:"); ui.add(egui::DragValue::new(&mut self.config.base_zoom).speed(0.01)); });
                    ui.horizontal(|ui| { ui.label("Width:"); ui.add(egui::DragValue::new(&mut self.config.base_width).speed(0.01)); });
                    ui.horizontal(|ui| { ui.label("Depth:"); ui.add(egui::DragValue::new(&mut self.config.base_depth).speed(1.0)); });
                    ui.horizontal(|ui| { ui.label("Brightness:"); ui.add(egui::DragValue::new(&mut self.config.base_brightness).speed(0.01)); });
                    if ui.small_button("Reset fractal defaults").clicked() { self.config.reset_fractal_to_default(); }
                }
            });

            ui.collapsing("🎛️ Audio Reactivity", |ui| {
                if self.visualizer_mode == VisualizerMode::UnknownPleasures {
                    ui.horizontal(|ui| { ui.label("Bass influence:"); ui.add(egui::DragValue::new(&mut self.config.up_bass_mult).speed(0.01)); });
                    ui.horizontal(|ui| { ui.label("Mid influence:"); ui.add(egui::DragValue::new(&mut self.config.up_mid_mult).speed(0.01)); });
                    ui.horizontal(|ui| { ui.label("Treble influence:"); ui.add(egui::DragValue::new(&mut self.config.up_treble_mult).speed(0.01)); });
                } else {
                    ui.horizontal(|ui| { ui.label("Bass → Zoom:"); ui.add(egui::DragValue::new(&mut self.config.zoom_bass_mult).speed(0.01)); });
                    ui.horizontal(|ui| { ui.label("Bass → Width:"); ui.add(egui::DragValue::new(&mut self.config.width_bass_mult).speed(0.01)); });
                    ui.horizontal(|ui| { ui.label("Complexity → Depth:"); ui.add(egui::DragValue::new(&mut self.config.depth_complexity_mult).speed(0.1)); });
                    ui.horizontal(|ui| { ui.label("Treble → Brightness:"); ui.add(egui::DragValue::new(&mut self.config.brightness_treble_mult).speed(0.01)); });
                }
            });

            ui.collapsing("✨ Animation", |ui| {
                ui.checkbox(&mut self.config.auto_rotate, "Auto Rotate");
                ui.horizontal(|ui| { ui.label("Rotation Speed:"); ui.add(egui::DragValue::new(&mut self.config.rotation_speed).speed(0.01)); });
                ui.checkbox(&mut self.config.pulse_on_beat, "Pulse on Beat");
                ui.checkbox(&mut self.config.color_cycle, "Color Cycle");
                ui.horizontal(|ui| { ui.label("Color Speed:"); ui.add(egui::DragValue::new(&mut self.config.color_cycle_speed).speed(0.01)); });
            });

            ui.collapsing("🖥️ Display", |ui| {
                ui.checkbox(&mut self.show_spectrum, "Show Spectrum");
                ui.checkbox(&mut self.show_waveform, "Show Waveform");
                ui.horizontal(|ui| { ui.label("Glow:"); ui.add(egui::DragValue::new(&mut self.config.glow_intensity).speed(0.01)); });
            });

            ui.separator();
            if ui.button("🔄 Reset Settings").clicked() { self.config = VisualizerConfig::default(); }
        });
    }
}

impl Default for MusicVisualizerNativeApp {
    fn default() -> Self {
        Self {
            audio: AudioAnalysis::new(),
            config: VisualizerConfig::default(),
            time: 0.0,
            rotation: 0.0,
            particles: Vec::new(),
            playlist: PlaylistState::default(),
            beat_flash: 0.0,
            visualizer_mode: VisualizerMode::Fractal,
            unknown_visualizer: UnknownPleasuresVisualizer::new(),
            audio_data: Arc::new(Mutex::new(Vec::new())),
            show_settings: false,
            show_spectrum: false,
            show_waveform: false,
            system_audio_mode: true,
            demo_mode: true,
            output_stream: None,
            output_stream_handle: None,
            current_sink: None,
            playback_start_instant: None,
            playback_seek_offset: 0.0,
        }
    }
}

impl eframe::App for MusicVisualizerNativeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let dt = ctx.input(|i| i.stable_dt);
        self.time += dt as f64;
    // Update audio analysis from CPAL buffer
    let audio_buf = self.audio_data.lock().unwrap().clone();
    self.audio.update_from_cpal(&audio_buf);
    // Update playback timing/state for file playback
    self.update_playback_state();
    // Update rotation and particles driven by audio
    let dt_f32 = dt as f32;
    if self.config.auto_rotate {
        // apply base rotation plus beat-influenced rotation multiplier
        self.rotation += self.config.rotation_speed * dt_f32 + self.audio.beat_intensity * self.config.rotation_beat_mult;
    }
    // spawn particles on beat
    if self.audio.beat {
        let spawn_count = (self.config.particle_count / 10).max(1) as usize;
        for _ in 0..spawn_count {
            let angle = rand::random::<f32>() * std::f32::consts::TAU;
            let speed = 30.0 + rand::random::<f32>() * 80.0;
            let color = self.get_current_color();
            self.particles.push(Particle::new(egui::Pos2::new(400.0, 300.0), angle, speed, color));
        }
    }
    // update and cull particles
    for p in &mut self.particles {
        p.update(dt_f32);
    }
    let target = self.config.particle_count as usize;
    self.particles.retain(|p| p.is_alive());
    if self.particles.len() > target { self.particles.truncate(target); }
        // Sidebar toggle
        if !self.show_settings {
            egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("▶ Show Settings").clicked() {
                        self.show_settings = true;
                    }
                });
            });
        }
        if self.show_settings {
            egui::SidePanel::left("settings_panel")
                .resizable(true)
                .default_width(280.0)
                .show(ctx, |ui| {
                    self.draw_settings_panel(ui);
                    if ui.button("◀ Hide").clicked() {
                        self.show_settings = false;
                    }
                });
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            let available = ui.available_rect_before_wrap();
            let spectrum_height = if self.show_spectrum { 80.0 } else { 0.0 };
            let waveform_height = if self.show_waveform { 60.0 } else { 0.0 };
            let bottom_ui_height = spectrum_height + waveform_height;
            let fractal_rect = egui::Rect::from_min_max(
                available.min,
                Pos2::new(available.max.x, available.max.y - bottom_ui_height),
            );
            match self.visualizer_mode {
                VisualizerMode::Fractal => self.draw_fractal(ui, fractal_rect),
                VisualizerMode::UnknownPleasures => {
                    self.unknown_visualizer.draw(ui, fractal_rect, &self.audio, &self.config, self.time);
                }
            }
            let painter = ui.painter();
            self.draw_particles(painter, fractal_rect.center());
            // Playback/update UI: draw compact play controls in corner
            let ctrl_rect = Rect::from_min_size(
                Pos2::new(fractal_rect.right() - 220.0, fractal_rect.top() + 8.0),
                egui::vec2(212.0, 48.0),
            );
            ui.allocate_rect(ctrl_rect, egui::Sense::hover());
            ui.painter().rect_filled(ctrl_rect, 4.0, Color32::from_rgba_unmultiplied(0, 0, 0, 100));
            ui.allocate_ui_at_rect(ctrl_rect.shrink(6.0), |ui| {
                ui.horizontal(|ui| {
                    if ui.small_button("⏮").clicked() { self.play_previous(); }
                    let play_icon = if self.playlist.is_playing { "⏸" } else { "▶" };
                    if ui.small_button(play_icon).clicked() { self.toggle_playback(); }
                    if ui.small_button("⏭").clicked() { self.play_next(); }
                    ui.separator();
                    ui.label(format!("{}", self.playlist.get_current_track().map(|t| t.name.clone()).unwrap_or_else(|| "No track".to_string())));
                });
            });
            if self.show_spectrum {
                let spectrum_rect = egui::Rect::from_min_max(
                    Pos2::new(available.min.x, available.max.y - bottom_ui_height),
                    Pos2::new(available.max.x, available.max.y - waveform_height),
                );
                ui.allocate_rect(spectrum_rect, egui::Sense::hover());
                self.draw_spectrum(ui, spectrum_rect);
            }
            if self.show_waveform {
                let waveform_rect = egui::Rect::from_min_max(
                    Pos2::new(available.min.x, available.max.y - waveform_height),
                    available.max,
                );
                ui.allocate_rect(waveform_rect, egui::Sense::hover());
                self.draw_waveform(ui, waveform_rect);
            }
        });
    }
}

// ===== Playback and playlist methods (native) =====
impl MusicVisualizerNativeApp {
    pub fn trigger_file_input(&mut self) {
        if let Some(paths) = FileDialog::new().add_filter("Audio", &["mp3", "wav", "ogg", "flac", "m4a"]).set_title("Select audio files").pick_files() {
            for p in paths {
                if let Some(name) = p.file_name().and_then(|s| s.to_str().map(|s| s.to_string())) {
                    let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
                    self.playlist.tracks.push(crate::playlist::PlaylistTrack { name: name.clone(), path: p.to_string_lossy().to_string(), duration: 0.0, file_type: ext.clone() });
                }
            }
            if self.playlist.current_index.is_none() && !self.playlist.tracks.is_empty() {
                self.playlist.current_index = Some(0);
            }
        }
    }

    fn ensure_output_stream(&mut self) {
        if self.output_stream.is_some() && self.output_stream_handle.is_some() {
            return;
        }
        if let Ok((stream, handle)) = OutputStream::try_default() {
            self.output_stream = Some(stream);
            self.output_stream_handle = Some(handle);
        }
    }

    pub fn play_track(&mut self, index: usize) {
        if index >= self.playlist.tracks.len() { return; }
        let track = self.playlist.tracks[index].clone();
        self.playlist.current_index = Some(index);
        self.playlist.is_playing = true;
        self.demo_mode = false;

        self.ensure_output_stream();

        if let Some(handle) = &self.output_stream_handle {
            // Stop previous sink
            if let Some(s) = self.current_sink.take() {
                s.stop();
            }

            if let Ok(file) = File::open(&track.path) {
                let buf = BufReader::new(file);
                if let Ok(decoder) = Decoder::new(buf) {
                    let sink = Sink::try_new(handle).unwrap();
                    // set volume
                    sink.set_volume(self.playlist.volume);
                    // try to get duration
                    if let Some(dur) = decoder.total_duration() {
                        self.playlist.duration = dur.as_secs_f64();
                    }
                    sink.append(decoder);
                    self.playback_start_instant = Some(Instant::now());
                    self.playback_seek_offset = 0.0;
                    self.current_sink = Some(sink);
                }
            }
        }
    }

    pub fn toggle_playback(&mut self) {
        if let Some(sink) = &self.current_sink {
            if self.playlist.is_playing {
                sink.pause();
                // adjust offset
                if let Some(start) = self.playback_start_instant {
                    let elapsed = start.elapsed().as_secs_f64();
                    self.playback_seek_offset += elapsed;
                }
                self.playback_start_instant = None;
                self.playlist.is_playing = false;
            } else {
                sink.play();
                self.playback_start_instant = Some(Instant::now());
                self.playlist.is_playing = true;
            }
        } else if self.playlist.current_index.is_some() {
            // no sink yet — start playing current
            let idx = self.playlist.current_index.unwrap();
            self.play_track(idx);
        }
    }

    pub fn stop_playback(&mut self) {
        if let Some(sink) = self.current_sink.take() {
            sink.stop();
        }
        self.playlist.is_playing = false;
        self.playlist.current_time = 0.0;
        self.playback_start_instant = None;
        self.playback_seek_offset = 0.0;
    }

    pub fn play_next(&mut self) {
        // try next index; if none, loop to 0
        let next = self.playlist.get_next_index().or_else(|| if !self.playlist.tracks.is_empty() { Some(0) } else { None });
        if let Some(idx) = next { self.play_track(idx); } else { self.stop_playback(); }
    }

    pub fn play_previous(&mut self) {
        if self.playlist.current_time > 3.0 {
            self.seek_to(0.0);
            return;
        }
        if let Some(prev) = self.playlist.get_prev_index() { self.play_track(prev); } else { self.stop_playback(); }
    }

    pub fn seek_to(&mut self, time: f64) {
        // Best-effort seeking: WAV files supported via `hound`; other formats restart at 0 and set time marker
        if let Some(idx) = self.playlist.current_index {
            let track = self.playlist.tracks[idx].clone();
            let ext = track.file_type.to_lowercase();
            // stop current sink
            if let Some(sink) = self.current_sink.take() {
                sink.stop();
            }

            self.ensure_output_stream();
            if let Some(handle) = &self.output_stream_handle {
                if ext == "wav" {
                    // Use hound to seek samples
                    if let Ok(mut reader) = hound::WavReader::open(&track.path) {
                        let spec = reader.spec();
                        let sample_rate = spec.sample_rate as u64;
                        let channels = spec.channels as usize;
                        let start_sample = (time * sample_rate as f64) as u64;
                        // skip to start_sample * channels
                        let total_samples = reader.duration();
                        let mut samples_iter = reader.samples::<i16>();
                        let to_skip = start_sample.saturating_mul(channels as u64);
                        for _ in 0..to_skip { let _ = samples_iter.next(); }
                        let samples: Vec<i16> = samples_iter.filter_map(Result::ok).collect();
                        // Create a SamplesBuffer and play
                        let source = rodio::buffer::SamplesBuffer::new(channels as u16, sample_rate as u32, samples);
                        let sink = Sink::try_new(handle).unwrap();
                        sink.set_volume(self.playlist.volume);
                        sink.append(source);
                        self.current_sink = Some(sink);
                        self.playback_start_instant = Some(Instant::now());
                        self.playback_seek_offset = time;
                        // duration: calculate from total_samples
                        if total_samples > 0 {
                            self.playlist.duration = (total_samples as f64) / (sample_rate as f64 * channels as f64);
                        }
                        self.playlist.is_playing = true;
                        self.playlist.current_time = time;
                        return;
                    }
                }

                // Fallback: start from beginning and set offset marker (approximate)
                if let Ok(file) = File::open(&track.path) {
                    let buf = BufReader::new(file);
                    if let Ok(decoder) = Decoder::new(buf) {
                        let sink = Sink::try_new(handle).unwrap();
                        sink.set_volume(self.playlist.volume);
                        if let Some(dur) = decoder.total_duration() {
                            self.playlist.duration = dur.as_secs_f64();
                        }
                        sink.append(decoder);
                        self.current_sink = Some(sink);
                        self.playback_start_instant = Some(Instant::now());
                        self.playback_seek_offset = time; // we note desired offset but actual audio will start at 0
                        self.playlist.is_playing = true;
                        self.playlist.current_time = time;
                    }
                }
            }
        }
    }

    pub fn update_volume(&mut self) {
        if let Some(sink) = &self.current_sink {
            sink.set_volume(self.playlist.volume);
        }
    }

    pub fn remove_track(&mut self, index: usize) {
        if index < self.playlist.tracks.len() {
            self.playlist.tracks.remove(index);
            if let Some(current) = self.playlist.current_index {
                if index == current {
                    self.stop_playback();
                    self.playlist.current_index = None;
                } else if index < current {
                    self.playlist.current_index = Some(current - 1);
                }
            }
        }
    }

    pub fn clear_playlist(&mut self) {
        self.stop_playback();
        self.playlist.tracks.clear();
        self.playlist.current_index = None;
    }

    pub fn update_playback_state(&mut self) {
        // Update current_time based on start instant + seek offset; detect end using sink.empty()
        if let Some(sink) = &self.current_sink {
            let elapsed = if let Some(start) = self.playback_start_instant {
                start.elapsed().as_secs_f64()
            } else { 0.0 };
            self.playlist.current_time = self.playback_seek_offset + elapsed;
            if sink.empty() && self.playlist.is_playing {
                // consider ended
                self.play_next();
            }
        }
    }
}

impl MusicVisualizerNativeApp {
    pub fn is_system_audio(&self) -> bool {
        self.system_audio_mode
    }

    pub fn set_system_audio(&mut self, enabled: bool) {
        self.system_audio_mode = enabled;
    }

    pub fn try_init_system_audio(&mut self) {
        // Native CPAL stream is configured in main; here we just log the attempt
        eprintln!("System audio capture requested: {}", self.system_audio_mode);
    }
}

// Drawing helpers and UI logic are in `src/ui.rs`.
