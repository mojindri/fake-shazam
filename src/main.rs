use crate::database::{get_db_couples, save_fingerprint};
use crate::fingerprint::generate_fingerprint;
use crate::input::load_mp3;
use crate::r#match::{find_matches};
use crate::other::generate_unique_id;
use crate::spectogram::{extract_peaks, spectogram};
use std::path::PathBuf;
use std::str::FromStr;

mod database;
mod fft;
mod fingerprint;
pub mod input;
mod r#match;
mod models;
mod other;
mod spectogram;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init(); // Initialize tracing
    train();
    // 1. Load audio file
    let audio_path = PathBuf::from_str("./sample/sample.mp3").unwrap();
    let (mut samples, sample_rate) = load_mp3(&audio_path)?;
    let sample_rate = sample_rate.unwrap();
    // 2. Trim to first 5 seconds
    let target_samples = sample_rate * 5;
    samples.truncate(target_samples as usize);

    // 3. Generate fingerprint
    let spectrogram = spectogram(&samples, sample_rate)?;
    let peaks = extract_peaks(&spectrogram, 5.0); // 5-second duration
    let fingerprint = generate_fingerprint(&peaks, 0); // Use 0 for sample ID
    let addresses: Vec<u32> = fingerprint.keys().copied().collect();
     let db_couples = get_db_couples(&addresses)?;
    // 4. Find matches
    let matches = find_matches(&fingerprint,&db_couples);

    // 5. Display results
    println!("Top matches: {}",matches.len());
    for m in matches.iter() {
        println!("Match: {} (score: {:.2}) {}", m.file_path.display(), m.score, m.timestamp);
    }

    Ok(())
}

fn train() {
    let mp3s = load_mp3_dir();
    for mp3 in mp3s {
        let (wave, sample_rate) = load_mp3(&mp3).unwrap();
        let sample_rate = sample_rate.unwrap();
        let spect = spectogram(&wave, sample_rate).unwrap();
        let peaks = extract_peaks(&spect, (wave.len() as u32 / sample_rate) as f64);
        tracing::info!("peaks {} founded.", peaks.len());
        let hash_ids = generate_fingerprint(&peaks, generate_unique_id());
        tracing::info!("hash ids {} generated.", hash_ids.len());
        save_fingerprint(
            &hash_ids,
            &format!("./hashes/{}", mp3.file_name().unwrap().to_string_lossy()),
        )
        .unwrap();
    }
}
fn load_mp3_dir() -> Vec<PathBuf> {
    let mut list = vec![];
    let mut fs = std::fs::read_dir("./mp3").unwrap();
    while let Some(Ok(item)) = &fs.next() {
        let path = item.path();
        //tracing::info!("Reading {:?}", path);
        list.push(path);
    }
    list
}
