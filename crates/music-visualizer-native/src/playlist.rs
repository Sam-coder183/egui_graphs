#[derive(Clone, Default)]
pub struct PlaylistTrack {
    pub name: String,
    pub path: String,
    pub duration: f64,
    pub file_type: String,
}

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
        if len == 0 { return; }
        self.shuffle_order = (0..len).collect();
        // simple Fisher-Yates using rand from std
        for i in (1..len).rev() {
            let j = (rand::random::<f32>() * (i as f32 + 1.0)) as usize;
            self.shuffle_order.swap(i, j);
        }
    }

    pub fn get_next_index(&self) -> Option<usize> {
        let len = self.tracks.len();
        if len == 0 { return None; }
        match self.current_index {
            Some(idx) => {
                if self.is_shuffled && !self.shuffle_order.is_empty() {
                    let pos = self.shuffle_order.iter().position(|&x| x == idx)?;
                    if pos + 1 < self.shuffle_order.len() { Some(self.shuffle_order[pos+1]) } else { None }
                } else {
                    if idx + 1 < len { Some(idx+1) } else { None }
                }
            }
            None => Some(0),
        }
    }

    pub fn get_prev_index(&self) -> Option<usize> {
        let len = self.tracks.len();
        if len == 0 { return None; }
        match self.current_index {
            Some(idx) => {
                if self.is_shuffled && !self.shuffle_order.is_empty() {
                    let pos = self.shuffle_order.iter().position(|&x| x == idx)?;
                    if pos > 0 { Some(self.shuffle_order[pos-1]) } else { None }
                } else {
                    if idx > 0 { Some(idx-1) } else { None }
                }
            }
            None => Some(0),
        }
    }
    // Add shuffle and navigation logic here as needed
}
