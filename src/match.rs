
use serde::{Deserialize, Serialize};

use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct Match {
    pub file_path: PathBuf,
    pub score: f64,
    pub timestamp: u32,
}

use num_complex::Complex;


#[derive(Debug, Clone)]
pub struct Peek {
    pub freq: Complex<f32>,
    pub time: f32,
    pub magnitude: f32,  // Added for potential magnitude weighting
}
use std::collections::HashMap;
use crate::models::Couple;


pub fn find_matches(
    sample_fingerprint: &HashMap<u32, Couple>,
    db_couples: &HashMap<u32, Vec<(Couple, PathBuf)>>,
) -> Vec<Match> {
    let mut song_data: HashMap<PathBuf, (Vec<(u32, u32)>, u32)> = HashMap::new();

    // Phase 1: Collect matches and track earliest timestamp
    for (address, sample_couple) in sample_fingerprint {
        if let Some(db_matches) = db_couples.get(address) {
            for (db_couple, path) in db_matches {
                // Store (sample_time, db_time) pairs
                let entry = song_data
                    .entry(path.clone())
                    .or_insert((Vec::new(), u32::MAX));

                entry.0.push((sample_couple.anchor_time_ms, db_couple.anchor_time_ms));

                // Track earliest timestamp
                if db_couple.anchor_time_ms < entry.1 {
                    entry.1 = db_couple.anchor_time_ms;
                }
            }
        }
    }

    // Phase 2: Calculate scores using pairwise comparisons
    let mut matches = Vec::with_capacity(song_data.len());
    for (path, (times, timestamp)) in song_data {
        let mut score = 0.0;

        // Original Go's O(n²) pairwise comparison
        for i in 0..times.len() {
            for j in (i + 1)..times.len() {
                let (s1, d1) = times[i];
                let (s2, d2) = times[j];

                let sample_diff = (s1 as f64 - s2 as f64).abs();
                let db_diff = (d1 as f64 - d2 as f64).abs();

                if (sample_diff - db_diff).abs() < 100.0 {
                    score += 1.0;
                }
            }
        }

        matches.push(Match {
            file_path: path,
            score,
            timestamp,
        });
    }

    // Sort descending by score (matches Go's sort order)
    matches.sort_unstable_by(|a, b| b.score.partial_cmp(&a.score).unwrap());

    matches
}