use num_complex::Complex;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const DSP_RATIO: u32 = 4; // use u32 if sample_rate is u32
pub const FREQ_BIN_SIZE: usize = 1024; // usize is handy for FFT buffer sizes
pub const MAX_FREQ: f32 = 5_000.0; // make this f32 so no cast later
pub const HOP_SIZE: usize = FREQ_BIN_SIZE / 32;

#[derive(Deserialize, Serialize)]
pub struct Couple {
    pub(crate) anchor_time_ms: u32,
    pub(crate) song_id: u32,
}
pub type Spectrogram = Vec<Vec<Complex<f64>>>;

#[derive(Clone, Copy, Debug)]
pub struct Peek {
    pub time: f64,      // In seconds
    pub freq_hz: f64,   // Actual frequency in Hz
    pub magnitude: f64, // Normalized magnitude
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Match {
    pub file_path: PathBuf,
    pub score: f64,
    pub timestamp: u32,
}
#[derive(Clone, Copy)]
pub struct Maxies {
    pub(crate) max_msg: f64,
    pub(crate) max_freq: Complex<f64>,
    pub(crate) freq_ids: i16,
}
pub struct Band {
    pub(crate) min: usize,
    pub(crate) max: usize,
}

/// Logarithmic frequency bands that match human hearing better
pub const BANDS: [Band; 7] = [
    Band { min: 0, max: 10 },    // 0-86 Hz (sub-bass)
    Band { min: 10, max: 20 },   // 86-172 Hz (bass)
    Band { min: 20, max: 40 },   // 172-344 Hz (low mids)
    Band { min: 40, max: 80 },   // 344-688 Hz (mids)
    Band { min: 80, max: 160 },  // 688-1375 Hz (upper mids)
    Band { min: 160, max: 320 }, // 1375-2750 Hz (presence)
    Band { min: 320, max: 512 }, // 2750-5000 Hz (brilliance)
];
