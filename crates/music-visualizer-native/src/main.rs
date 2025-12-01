mod audio_analysis;
mod visualizer_config;
mod playlist;
mod particle;
mod unknown_pleasures;
mod app;
mod ui;

use eframe::NativeOptions;
use std::sync::{Arc, Mutex};
use crate::app::MusicVisualizerNativeApp;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

fn main() {
    let audio_data = Arc::new(Mutex::new(vec![0.0; 512]));
    start_cpal_stream(audio_data.clone());
    let app = MusicVisualizerNativeApp::with_audio_data(audio_data);
    let native_options = NativeOptions::default();
    if let Err(e) = eframe::run_native(
        "Music Visualizer Native",
        native_options,
        Box::new(|_cc| Box::new(app)),
    ) {
        eprintln!("Failed to start native app: {e}");
    }
}

fn start_cpal_stream(audio_data: Arc<Mutex<Vec<f32>>>) {
    let host = cpal::default_host();
    let device = host.default_input_device().expect("No input device available");
    let config = device.default_input_config().unwrap();
    let err_fn = |err| eprintln!("CPAL stream error: {}", err);
    let timeout = None; // Option<Duration>
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => device.build_input_stream(
            &config.into(),
            move |data: &[f32], _| {
                let mut audio = audio_data.lock().unwrap();
                for (i, sample) in data.iter().enumerate().take(audio.len()) {
                    audio[i] = *sample;
                }
            },
            err_fn,
            timeout,
        ),
        cpal::SampleFormat::I16 => device.build_input_stream(
            &config.into(),
            move |data: &[i16], _| {
                let mut audio = audio_data.lock().unwrap();
                for (i, sample) in data.iter().enumerate().take(audio.len()) {
                    audio[i] = *sample as f32 / i16::MAX as f32;
                }
            },
            err_fn,
            timeout,
        ),
        cpal::SampleFormat::U16 => device.build_input_stream(
            &config.into(),
            move |data: &[u16], _| {
                let mut audio = audio_data.lock().unwrap();
                for (i, sample) in data.iter().enumerate().take(audio.len()) {
                    audio[i] = *sample as f32 / u16::MAX as f32;
                }
            },
            err_fn,
            timeout,
        ),
        _ => panic!("Unsupported sample format"),
    };
    stream.unwrap().play().unwrap();
}
