use crate::models::Couple;
use crate::spectogram::Peek;
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
pub fn create_address(anchor: &Peek, target: &Peek) -> u32 {
    // 1. Extract frequencies using real parts (matches Go's real() usage)
    let anchor_freq = anchor.freq.re as u32; // Convert to u32 directly
    let target_freq = target.freq.re as u32; // Critical fix: use target.freq

    // 2. Calculate time delta in milliseconds (exact Go conversion)
    let delta_ms = ((target.time - anchor.time) * 1000.0) as u32;

    // 3. Bitwise composition (matches Go's << and | operations)
    (anchor_freq << 23) | (target_freq << 14) | (delta_ms & 0x3FFF)
}

#[cfg(test)]
mod tests {

    use crate::fingerprint::{create_address, generate_fingerprint};
    use crate::spectogram::Peek;
    use num_complex::Complex;

    fn complex_from_polar(r: f64, theta: f64) -> Complex<f64> {
        Complex::new(r * theta.cos(), r * theta.sin())
    }
    #[test]
    fn test_create_address() {
        let anchor = Peek {
            time: 1.5,
            freq: Complex::new(100.0, 0.0), // Anchor frequency = 100
        };

        let target = Peek {
            time: 1.8,                      // 0.3s difference
            freq: Complex::new(200.0, 0.0), // Target frequency = 200
        };

        // Proper calculation:
        // 100 << 23 = 0x64000000
        // 200 << 14 = 0x00032000
        // 300ms      = 0x0000012C
        // Combined:  0x6432012C = 1,680,470,316
        let expected = (100u32 << 23) | (200u32 << 14) | 300u32;

        assert_eq!(create_address(&anchor, &target), expected);
    }

    #[test]
    fn respects_target_zone_size() {
        let peaks = (0..10)
            .map(|i| test_peak(100.0 + i as f64, 1.0 + i as f64 * 0.1))
            .collect::<Vec<_>>();

        let fp = generate_fingerprint(&peaks, 111);

        // Each peak should pair with next 5 peaks (or until end)
        // Total pairs: 5+5+5+5+5+4+3+2+1+0 = 35
        assert_eq!(fp.len(), 35);
    }

    fn test_peak(freq: f64, time: f64) -> Peek {
        Peek {
            time,
            freq: Complex::new(freq, 0.0),
        }
    }
    #[test]
    fn generates_correct_number_of_fingerprints() {
        let peaks = vec![
            test_peak(100.0, 1.0),
            test_peak(200.0, 1.1),
            test_peak(300.0, 1.2),
        ];

        let fp = generate_fingerprint(&peaks, 123);
        // Each peak pairs with following peaks within target zone:
        // Peak 0 → 1,2
        // Peak 1 → 2
        // Peak 2 → none
        assert_eq!(fp.len(), 3);
    }
}
