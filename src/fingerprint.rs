use crate::models::{Couple, Peek};

use std::cmp::min;
use std::collections::HashMap;

const TARGET_ZONE_SIZE: usize = 5;
pub fn generate_fingerprint(peaks: &[Peek], song_id: u32) -> HashMap<u32, Couple> {
    let mut fingerprints = HashMap::new();
    // tracing::info!("Generating fingerprint with {} peaks", peaks.len());
    for (i, anchor) in peaks.iter().enumerate() {
        let max_j = min(peaks.len(), i + TARGET_ZONE_SIZE + 1);
        for j in (i + 1)..max_j {
            let target = &peaks[j];
            let address = create_address(anchor, target);
            let anchor_time_ms = (anchor.time * 1000.0) as u32;
            fingerprints.insert(
                address,
                Couple {
                    anchor_time_ms,
                    song_id,
                },
            );
        }
    }
    fingerprints
}
// fingerprint.rs
pub fn create_address(anchor: &Peek, target: &Peek) -> u32 {
    // Convert frequencies to virtual bins (0-511) assuming 5000Hz max
    let anchor_bin = ((anchor.freq_hz / 5000.0) * 511.0).round() as u32;
    let target_bin = ((target.freq_hz / 5000.0) * 511.0).round() as u32;

    // Clamp values to valid ranges
    let anchor_freq = anchor_bin.min(511);
    let target_freq = target_bin.min(511);
    let delta_ms = ((target.time - anchor.time) * 1000.0).clamp(0.0, 16383.0) as u32;

    // Original bitwise composition
    (anchor_freq << 23) | (target_freq << 14) | (delta_ms & 0x3FFF)
}
