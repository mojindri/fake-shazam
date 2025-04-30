use std::cmp::Ordering;
use crate::database::get_db_couples;
use crate::models::Couple;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct Match {
    pub file_path: PathBuf,
    pub score: f64,
    pub timestamp: u32,
}
pub fn find_matches(
    sample_fingerprint: &HashMap<u32, Couple>,
    db_couples: HashMap<u32, Vec<(Couple, PathBuf)>>,
) -> anyhow::Result<Vec<Match>> {
    println!("Sample fingerprint contains {} addresses", sample_fingerprint.len());
    println!("Database contains {} addresses", db_couples.len());
    let mut matches: HashMap<u32, Vec<(u32, u32)>> = HashMap::new();
    let mut timestamps: HashMap<u32, u32> = HashMap::new();
    let mut song_paths: HashMap<u32, PathBuf> = HashMap::new();

    // Collect matches and metadata
    for (address, sample_couple) in sample_fingerprint {
        let sample_time = sample_couple.anchor_time_ms;
        if let Some(entries) = db_couples.get(address) {
            for (db_couple, path) in entries {
                let song_id = db_couple.song_id;
                let db_time = db_couple.anchor_time_ms;

                // Update matches
                matches
                    .entry(song_id)
                    .or_default()
                    .push((sample_time, db_time));

                // Update earliest timestamp
                timestamps
                    .entry(song_id)
                    .and_modify(|e| {
                        if db_time < *e {
                            *e = db_time;
                        }
                    })
                    .or_insert(db_time);

                // Store first encountered path for metadata
                song_paths.entry(song_id).or_insert_with(|| path.clone());
            }
        }
    }

    // Calculate scores
    let scores = analyze_relative_timing(&matches);

    // Build match results
    let mut match_list = Vec::new();
    for (song_id, score) in scores {
        let timestamp = timestamps.get(&song_id).copied().unwrap_or(0);
        let path = song_paths.get(&song_id).cloned().unwrap_or_default();


        match_list.push(Match {
            file_path: path,
            timestamp,
            score,
        });
    }

    // Sort matches by descending score
    match_list.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal));

    Ok(match_list)
}
// Update the analyze_relative_timing function
fn analyze_relative_timing(matches: &HashMap<u32, Vec<(u32, u32)>>) -> HashMap<u32, f64> {
    let mut scores = HashMap::new();
    for (song_id, times) in matches {
        // Base score for each matching pair
        let mut score = times.len() as f64 * 0.5;

        // Bonus for temporal consistency
        for i in 0..times.len() {
            for j in (i + 1)..times.len() {
                let (s_i, db_i) = times[i];
                let (s_j, db_j) = times[j];

                let sample_diff = s_i.abs_diff(s_j);
                let db_diff = db_i.abs_diff(db_j);

                if sample_diff.abs_diff(db_diff) < 100 {
                    score += 1.0;
                }
            }
        }

        scores.insert(*song_id, score);
    }
    scores
}

pub fn find_matches_basic(sample_fingerprint: &HashMap<u32, Couple> ,    db_couples: HashMap<u32, Vec<(Couple, PathBuf)>>,) -> anyhow::Result<Vec<Match>> {

    let mut file_scores = HashMap::new();

    for (address, couples_with_paths) in &db_couples {
        if let Some(sample_couple) = sample_fingerprint.get(address) {
            for (db_couple, audio_path) in couples_with_paths {
                // Calculate time delta score
                let time_diff =
                    (sample_couple.anchor_time_ms as i32 - db_couple.anchor_time_ms as i32).abs();

                // Simple scoring: lower time difference = higher score
                let score = 1.0 / (time_diff as f64 + 1.0);

                file_scores
                    .entry(audio_path)
                    .and_modify(|s| *s += score)
                    .or_insert(score);
            }
        }
    }

    // Convert to sorted matches
    let mut match_list: Vec<Match> = file_scores
        .into_iter()
        .map(|(path, score)| Match {
            file_path: path.clone(),
            score,
            timestamp: 0,
        })
        .collect();

    match_list.sort_unstable_by(|a, b| b.score.partial_cmp(&a.score).unwrap());

    Ok(match_list)
}



// Update the test to match new scoring
#[test]
fn test_basic_matching() {
    let mut sample_fp = HashMap::new();
    sample_fp.insert(0x12345678, Couple { anchor_time_ms: 1000, song_id: 0 });

    let mut db = HashMap::new();
    let x = PathBuf::from("test.mp3");
    db.insert(0x12345678, vec![
        (Couple { anchor_time_ms: 1005, song_id: 1 }, x.clone())
    ]);

    let matches = find_matches(&sample_fp, db).unwrap();
    assert!(!matches.is_empty(), "Should find at least one match");
    assert_eq!(
        matches[0].file_path.file_name(),
        PathBuf::from("test.mp3").file_name()
    );
    // Base score (0.5) + no temporal matches (0) = 0.5
    assert_eq!(matches[0].score, 0.5);
}

// Add a test with multiple matches
#[test]
fn test_temporal_matching() {
    let mut sample_fp = HashMap::new();
    sample_fp.insert(0x12345678, Couple { anchor_time_ms: 1000, song_id: 0 });
    sample_fp.insert(0x12345679, Couple { anchor_time_ms: 2000, song_id: 0 });

    let mut db = HashMap::new();
    let x = PathBuf::from("test.mp3");
    db.insert(0x12345678, vec![
        (Couple { anchor_time_ms: 1005, song_id: 1 }, x.clone())
    ]);
    db.insert(0x12345679, vec![
        (Couple { anchor_time_ms: 2005, song_id: 1 }, x.clone())
    ]);

    let matches = find_matches(&sample_fp, db).unwrap();
    assert!(!matches.is_empty());
    // Base score (2 * 0.5 = 1.0) + temporal match (1.0) = 2.0
    assert_eq!(matches[0].score, 2.0);
}
#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;
    use crate::models::Couple;
    use crate::r#match::find_matches;

}