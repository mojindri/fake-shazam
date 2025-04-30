use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct Couple {
    pub(crate) anchor_time_ms: u32,
    pub(crate) song_id: u32,
}
