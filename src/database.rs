use crate::models::Couple;
use anyhow::Context;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::fs::File;
use std::io::{BufReader, Write};
use std::path::{Path, PathBuf};

// Modified database loading to track origin paths
pub fn get_db_couples(addresses: &[u32]) -> anyhow::Result<HashMap<u32, Vec<(Couple, PathBuf)>>> {
    let mut db = HashMap::new();
    let address_set: HashSet<u32> = addresses.iter().copied().collect();

    let hash_dir = Path::new("./hashes");
    if !hash_dir.is_dir() {
        anyhow::bail!("Hashes directory not found");
    }

    for entry in fs::read_dir(hash_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().is_some_and(|ext| ext == "mp3") {
            let file = File::open(&path)
                .with_context(|| format!("Failed to open file: {}", path.display()))?;

            let reader = BufReader::new(file);
            let fingerprint: HashMap<u32, Couple> = serde_json::from_reader(reader)
                .with_context(|| format!("Failed to parse JSON in: {}", path.display()))?;

            // Convert JSON path to audio path
            let audio_path = path.with_extension("mp3").to_path_buf();

            for (address, couple) in fingerprint {
                if address_set.contains(&address) {
                    db.entry(address)
                        .or_insert_with(Vec::new)
                        .push((couple, audio_path.clone()));
                }
            }
        }
    }

    tracing::info!("Loaded {} hash entries", db.len());
    Ok(db)
}
pub fn save_fingerprint(fingerprint: &HashMap<u32, Couple>, path: &str) -> std::io::Result<()> {
    let json = serde_json::to_string(fingerprint)?;
    let mut file = File::create(path)?;
    file.write_all(json.as_bytes())?;
    Ok(())
}

pub fn load_fingerprint(path: &str) -> std::io::Result<HashMap<u32, Couple>> {
    let file = File::open(path)?;
    let fingerprint = serde_json::from_reader(file)?;
    Ok(fingerprint)
}
