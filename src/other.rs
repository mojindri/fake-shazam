use rand::{Rng, rng};

pub fn generate_unique_id() -> u32 {
    let mut rng = rng();
    rng.random()
}
