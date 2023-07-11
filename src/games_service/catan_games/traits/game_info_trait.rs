use serde::{Deserialize, Serialize};

use crate::games_service::{harbors::harbor::HarborData, tiles::tile_enums::TileResource};

pub trait GameInfoTrait: Serialize + Deserialize<'static> {
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
