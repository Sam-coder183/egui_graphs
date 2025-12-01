// Native audio analysis struct, adapted from WASM version
// This implementation computes basic band averages and a simple spectral flux
// so fields like low_mid, high_mid and spectral_flux are actively used.
#[derive(Clone, Default)]
pub struct AudioAnalysis {
    pub bass: f32,
    pub low_mid: f32,
    pub mid: f32,
    pub high_mid: f32,
    pub treble: f32,
    pub volume: f32,
    pub peak: f32,
    pub beat: bool,
    pub beat_intensity: f32,
    pub spectral_centroid: f32,
    pub spectral_flux: f32,
    pub smooth_bass: f32,
    pub smooth_mid: f32,
    pub smooth_treble: f32,
    pub smooth_volume: f32,
    pub frequency_data: Vec<f32>,
    pub time_data: Vec<f32>,
    // previous frequency snapshot used to compute spectral flux
    pub prev_frequency_data: Vec<f32>,
}

impl AudioAnalysis {
    pub fn new() -> Self {
        Self {
            frequency_data: vec![0.0; 256],
            time_data: vec![0.0; 256],
            prev_frequency_data: vec![0.0; 256],
            ..Default::default()
        }
    }
    pub fn update_from_cpal(&mut self, buffer: &[f32]) {
        // Fill time_data and frequency_data with buffer values
        let len = buffer.len();
        if len == 0 {
            return;
        }
        // For now, treat buffer as time domain
        self.time_data = buffer.iter().cloned().collect();
        // Simple RMS volume
        let rms = (buffer.iter().map(|x| x * x).sum::<f32>() / len as f32).sqrt();
        self.volume = rms;
        self.smooth_volume = self.smooth_volume + (rms - self.smooth_volume) * 0.15;
        // Peak detection
        self.peak = buffer.iter().map(|x| x.abs()).fold(0.0, f32::max);
        // Simulate frequency_data (for now, just copy time_data)
        let new_freq: Vec<f32> = buffer.iter().map(|x| (x.abs() * 255.0).min(255.0)).collect();
        // compute spectral flux against previous frame
        let mut flux = 0.0f32;
        let prev = &self.prev_frequency_data;
        for i in 0..new_freq.len() {
            let prev_v = if i < prev.len() { prev[i] } else { 0.0 };
            let diff = new_freq[i] - prev_v;
            if diff > 0.0 { flux += diff; }
        }
        self.spectral_flux = flux;
        self.frequency_data = new_freq;
        // Basic band analysis
        let band_size = len / 8; // finer division
        let bass_range = 0..band_size.max(1);
        let low_mid_range = band_size..(band_size * 2).max(band_size + 1);
        let mid_range = (band_size * 2)..(band_size * 4).max((band_size * 2) + 1);
        let high_mid_range = (band_size * 4)..(band_size * 6).min(len);
        let treble_range = (band_size * 6)..len;

        let avg = |r: std::ops::Range<usize>| -> f32 {
            if r.is_empty() { return 0.0; }
            let s: f32 = buffer[r.clone()].iter().map(|x| x.abs()).sum();
            s / (r.end - r.start) as f32
        };

        self.bass = avg(bass_range.clone());
        self.low_mid = avg(low_mid_range.clone());
        self.mid = avg(mid_range.clone());
        self.high_mid = avg(high_mid_range.clone());
        self.treble = avg(treble_range.clone());
        // Smooth bands
        self.smooth_bass = self.smooth_bass + (self.bass - self.smooth_bass) * 0.15;
        self.smooth_mid = self.smooth_mid + (self.mid - self.smooth_mid) * 0.15;
        self.smooth_treble = self.smooth_treble + (self.treble - self.smooth_treble) * 0.15;
        // Beat detection (simple energy spike)
        let energy_jump = self.bass - self.smooth_bass;
        self.beat = energy_jump > 0.1 && self.bass > 0.3;
        self.beat_intensity = if self.beat { energy_jump.min(1.0) } else { 0.0 };

        // store frequency snapshot for next frame spectral flux calculation
        self.prev_frequency_data = self.frequency_data.clone();
    }
}
