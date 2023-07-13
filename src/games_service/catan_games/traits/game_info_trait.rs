use crate::games_service::{harbors::harbor::HarborData, tiles::tile_enums::TileResource};

use rand::{thread_rng, Rng};

pub trait GameInfoTrait {
    fn name(&self) -> &str;
    fn tile_resources(&self) -> &[TileResource];
    fn rolls(&self) -> &[u32];
    fn rows_per_column(&self) -> &[u32];
    fn harbor_data(&self) -> &[HarborData];
}
#[macro_export]
macro_rules! harbor_data {
    ($tile_key:expr, $position:expr, $harbor_type:expr) => {
        HarborData::new(HarborKey::new($tile_key, $position), $harbor_type)
    };
}

/**
 * Shuffles the elements of a vector in place using the Fisher-Yates shuffle algorithm.
 * @param vector The vector to shuffle.
 * @returns The same vector with its elements shuffled.
 */
pub fn shuffle_vector<T>(vector: &mut Vec<T>) {
    let mut rng = thread_rng();
    let len = vector.len();
    for i in (1..len).rev() {
        let j = rng.gen_range(0..=i);
        vector.swap(i, j);
    }
}
