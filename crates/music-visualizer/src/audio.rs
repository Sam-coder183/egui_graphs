use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

// Audio analysis data extracted from Web Audio API
#[derive(Clone, Default)]
pub struct AudioAnalysis {
    // Frequency bands (normalized 0.0-1.0)
    pub bass: f32,         // 20-250 Hz
    pub low_mid: f32,      // 250-500 Hz
    pub mid: f32,          // 500-2000 Hz
    pub high_mid: f32,     // 2000-4000 Hz
    pub treble: f32,       // 4000-20000 Hz

    // Overall metrics
    pub volume: f32,       // RMS volume
    pub peak: f32,         // Peak amplitude

    // Beat detection
    pub beat: bool,        // True when beat detected
    pub beat_intensity: f32,

    // Spectral features
    pub spectral_centroid: f32,
    pub spectral_flux: f32,

    // Smoothed values for animation
    pub smooth_bass: f32,
    pub smooth_mid: f32,
    pub smooth_treble: f32,
    pub smooth_volume: f32,

    // Raw frequency data
    pub frequency_data: Vec<u8>,
    pub time_data: Vec<u8>,
}

impl AudioAnalysis {
    pub fn new() -> Self {
        Self {
            frequency_data: vec![0u8; 256],
            time_data: vec![0u8; 256],
            ..Default::default()
        }
    }

    pub fn update_from_fft(&mut self, frequency_data: &[u8], time_data: &[u8]) {
        self.frequency_data = frequency_data.to_vec();
        self.time_data = time_data.to_vec();

        let len = frequency_data.len();
        if len == 0 {
            return;
        }

        // Calculate frequency bands
        let bass_range = 0..len / 16;
        let low_mid_range = len / 16..len / 8;
        let mid_range = len / 8..len / 4;
        let high_mid_range = len / 4..len / 2;
        let treble_range = len / 2..len;

        let calc_band_avg = |range: std::ops::Range<usize>| -> f32 {
            if range.is_empty() {
                return 0.0;
            }
            let sum: u32 = frequency_data[range.clone()].iter().map(|&x| x as u32).sum();
            (sum as f32) / (range.len() as f32 * 255.0)
        };

        let new_bass = calc_band_avg(bass_range);
        let new_low_mid = calc_band_avg(low_mid_range);
        let new_mid = calc_band_avg(mid_range);
        let new_high_mid = calc_band_avg(high_mid_range);
        let new_treble = calc_band_avg(treble_range);

        // Calculate volume (RMS)
        let rms: f32 = (time_data.iter()
            .map(|&x| {
                let centered = (x as f32) - 128.0;
                centered * centered
            })
            .sum::<f32>() / time_data.len() as f32)
            .sqrt() / 128.0;

        // Peak detection
        let peak = time_data.iter()
            .map(|&x| ((x as f32) - 128.0).abs())
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(0.0) / 128.0;

        // Beat detection (energy spike in bass)
        let bass_threshold = 0.6;
        let energy_jump = new_bass - self.smooth_bass;
        self.beat = energy_jump > 0.1 && new_bass > bass_threshold;
        self.beat_intensity = if self.beat { energy_jump.min(1.0) } else { 0.0 };

        // Spectral centroid (brightness)
        let total_energy: f32 = frequency_data.iter().map(|&x| x as f32).sum();
        if total_energy > 0.0 {
            let weighted_sum: f32 = frequency_data.iter()
                .enumerate()
                .map(|(i, &x)| (i as f32) * (x as f32))
                .sum();
            self.spectral_centroid = weighted_sum / total_energy / len as f32;
        }

        // Spectral flux (change in spectrum)
        let flux: f32 = frequency_data.iter()
            .zip(self.frequency_data.iter())
            .map(|(&new, &old)| {
                let diff = (new as f32) - (old as f32);
                if diff > 0.0 { diff } else { 0.0 }
            })
            .sum::<f32>() / (len as f32 * 255.0);
        self.spectral_flux = flux;

        // Smooth transitions
        let smoothing = 0.15;
        self.smooth_bass = self.smooth_bass + (new_bass - self.smooth_bass) * smoothing;
        self.smooth_mid = self.smooth_mid + (new_mid - self.smooth_mid) * smoothing;
        self.smooth_treble = self.smooth_treble + (new_treble - self.smooth_treble) * smoothing;
        self.smooth_volume = self.smooth_volume + (rms - self.smooth_volume) * smoothing;

        // Update raw values
        self.bass = new_bass;
        self.low_mid = new_low_mid;
        self.mid = new_mid;
        self.high_mid = new_high_mid;
        self.treble = new_treble;
        self.volume = rms;
        self.peak = peak;
    }

    // Demo mode with simulated audio
    pub fn simulate_demo(&mut self, time: f64) {
        // Simulate bass beat
        let beat_freq = 2.0; // BPM / 60
        let beat_phase = (time * beat_freq * std::f64::consts::TAU).sin();
        let beat_envelope = ((beat_phase + 1.0) / 2.0).powf(4.0) as f32;

        self.bass = 0.3 + beat_envelope * 0.5;
        self.low_mid = 0.25 + (time * 1.5).sin() as f32 * 0.15;
        self.mid = 0.3 + (time * 2.3).sin() as f32 * 0.2;
        self.high_mid = 0.2 + (time * 3.7).sin() as f32 * 0.15;
        self.treble = 0.15 + (time * 5.1).sin() as f32 * 0.1;

        self.volume = 0.4 + beat_envelope * 0.3;
        self.peak = self.volume * 1.2;

        self.beat = beat_envelope > 0.8;
        self.beat_intensity = if self.beat { beat_envelope } else { 0.0 };

        self.spectral_centroid = 0.5 + (time * 0.5).sin() as f32 * 0.3;
        self.spectral_flux = beat_envelope * 0.5;

        // Smooth values
        let smoothing = 0.1;
        self.smooth_bass = self.smooth_bass + (self.bass - self.smooth_bass) * smoothing;
        self.smooth_mid = self.smooth_mid + (self.mid - self.smooth_mid) * smoothing;
        self.smooth_treble = self.smooth_treble + (self.treble - self.smooth_treble) * smoothing;
        self.smooth_volume = self.smooth_volume + (self.volume - self.smooth_volume) * smoothing;

        // Generate demo frequency/time data
        for i in 0..self.frequency_data.len() {
            let freq_norm = i as f64 / self.frequency_data.len() as f64;
            let value = ((1.0 - freq_norm).powf(2.0) * self.bass as f64 * 200.0
                + (time * (10.0 + i as f64 * 0.5)).sin().abs() * 50.0) as u8;
            self.frequency_data[i] = value;
        }

        for i in 0..self.time_data.len() {
            let t = i as f64 / self.time_data.len() as f64;
            let wave = (t * std::f64::consts::TAU * 4.0 + time * 10.0).sin();
            let value = 128.0 + wave * 64.0 * self.volume as f64;
            self.time_data[i] = value.clamp(0.0, 255.0) as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simulate_demo_updates_fields() {
        let mut a = AudioAnalysis::new();
        a.simulate_demo(0.1);
        // After simulation some fields should be non-zero and within expected ranges
        assert!(a.bass >= 0.0 && a.bass <= 1.0);
        assert!(a.volume >= 0.0 && a.volume <= 1.5);
        assert!(a.frequency_data.len() > 0);
        assert!(a.time_data.len() > 0);
    }

    #[test]
    fn update_from_fft_handles_empty() {
        let mut a = AudioAnalysis::new();
        // Should not panic on empty slices
        a.update_from_fft(&[], &[]);
        assert_eq!(a.frequency_data.len(), 0);
        assert_eq!(a.time_data.len(), 0);
    }
}

// Web Audio wrapper (placeholder for future expansion)
#[allow(dead_code)]
#[derive(Clone)]
pub struct WebAudio {
    pub initialized: bool,
    pub error_message: Option<String>,
}

impl Default for WebAudio {
    fn default() -> Self {
        Self {
            initialized: false,
            error_message: None,
        }
    }
}

// Initialize Web Audio API
pub async fn init_web_audio(audio_data: Rc<RefCell<(Vec<u8>, Vec<u8>)>>) -> Result<(), JsValue> {
    let window = web_sys::window().ok_or("No window")?;
    let navigator = window.navigator();
    let media_devices = navigator.media_devices()?;

    // Request microphone access
    let constraints = web_sys::MediaStreamConstraints::new();
    constraints.set_audio(&JsValue::TRUE);
    constraints.set_video(&JsValue::FALSE);

    let promise = media_devices.get_user_media_with_constraints(&constraints)?;
    let stream: web_sys::MediaStream = wasm_bindgen_futures::JsFuture::from(promise).await?.into();

    // Create audio context and analyser
    let audio_ctx = web_sys::AudioContext::new()?;
    let analyser = audio_ctx.create_analyser()?;
    analyser.set_fft_size(512);
    analyser.set_smoothing_time_constant(0.8);

    let source = audio_ctx.create_media_stream_source(&stream)?;
    source.connect_with_audio_node(&analyser)?;

    // Store analyser for later use (simplified - in production would use more robust pattern)
    let freq_data_length = analyser.frequency_bin_count() as usize;
    let time_data_length = analyser.fft_size() as usize;

    // Set up animation frame callback
    let audio_data_clone = audio_data.clone();
    let analyser_clone = analyser.clone();

    let callback = Closure::wrap(Box::new(move || {
        let mut freq_data = vec![0u8; freq_data_length];
        let mut time_data = vec![0u8; time_data_length];

        analyser_clone.get_byte_frequency_data(&mut freq_data);
        analyser_clone.get_byte_time_domain_data(&mut time_data);

        *audio_data_clone.borrow_mut() = (freq_data, time_data);
    }) as Box<dyn Fn()>);

    // Start polling audio data
    let window_clone = window.clone();
    fn request_frame(window: &web_sys::Window, callback: &Closure<dyn Fn()>) {
        window.set_interval_with_callback_and_timeout_and_arguments_0(
            callback.as_ref().unchecked_ref(),
            16, // ~60fps
        ).ok();
    }

    request_frame(&window_clone, &callback);
    callback.forget(); // Leak the closure to keep it alive

    Ok(())
}

// No re-exports here. Types are available via the module path (crate::audio::AudioAnalysis, etc.).
