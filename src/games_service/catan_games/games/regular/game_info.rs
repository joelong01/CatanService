use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

use crate::{
    games_service::{
        catan_games::{game_enums::Direction, traits::game_info_trait::GameInfoTrait},
        harbors::{harbor::HarborData, harbor_enums::HarborType, harbor_key::HarborKey},
        tiles::{tile_enums::TileResource, tile_key::TileKey},
    },
    harbor_data,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegularGameInfo {
    pub name: String,
    pub tile_resources: Vec<TileResource>,
    pub rolls: Vec<u32>,
    pub rows_per_column: Vec<u32>,
    pub harbor_data: Vec<HarborData>,
}

impl GameInfoTrait for RegularGameInfo {
    fn name(&self) -> &str {
        &self.name
    }

    fn tile_resources(&self) -> &[TileResource] {
        &self.tile_resources
    }

    fn rolls(&self) -> &[u32] {
        &self.rolls
    }

    fn rows_per_column(&self) -> &[u32] {
        &self.rows_per_column
    }

    fn harbor_data(&self) -> &[HarborData] {
        &self.harbor_data
    }
}
fn create_regular_game_info() -> RegularGameInfo {
    RegularGameInfo {
        name: "Regular Game".to_owned(),
        tile_resources: vec![
            TileResource::Brick,
            TileResource::Brick,
            TileResource::Brick,
            TileResource::Desert,
            TileResource::Ore,
            TileResource::Ore,
            TileResource::Ore,
            TileResource::Sheep,
            TileResource::Sheep,
            TileResource::Sheep,
            TileResource::Sheep,
            TileResource::Wheat,
            TileResource::Wheat,
            TileResource::Wheat,
            TileResource::Wheat,
            TileResource::Wood,
            TileResource::Wood,
            TileResource::Wood,
            TileResource::Wood,
        ],
        rolls: vec![2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12],
        rows_per_column: vec![3, 4, 5, 4, 3],
        harbor_data: vec![
            harbor_data!(
                TileKey::new(-2, 1, 1),
                Direction::SouthWest,
                HarborType::Wheat
            ),
            harbor_data!(TileKey::new(0, 2, -2), Direction::South, HarborType::Brick),
            harbor_data!(
                TileKey::new(2, -2, 0),
                Direction::NorthEast,
                HarborType::Sheep
            ),
            harbor_data!(
                TileKey::new(2, -1, -1),
                Direction::SouthEast,
                HarborType::Ore
            ),
            harbor_data!(
                TileKey::new(1, 1, -2),
                Direction::SouthEast,
                HarborType::ThreeForOne
            ),
            harbor_data!(
                TileKey::new(1, -2, 1),
                Direction::North,
                HarborType::ThreeForOne
            ),
            harbor_data!(
                TileKey::new(-1, -1, 2),
                Direction::North,
                HarborType::ThreeForOne
            ),
            harbor_data!(
                TileKey::new(-1, 2, -1),
                Direction::SouthWest,
                HarborType::ThreeForOne
            ),
            harbor_data!(
                TileKey::new(-2, 0, 2),
                Direction::NorthWest,
                HarborType::Wood
            ),
        ],
    }
}

pub static REGULAR_GAME_INFO: Lazy<RegularGameInfo> = Lazy::new(|| create_regular_game_info());

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegularGame;

impl GameInfoTrait for RegularGame {
    fn name(&self) -> &str {
        &REGULAR_GAME_INFO.name
    }

    fn tile_resources(&self) -> &[TileResource] {
        &REGULAR_GAME_INFO.tile_resources
    }

    fn rolls(&self) -> &[u32] {
        &REGULAR_GAME_INFO.rolls
    }

    fn rows_per_column(&self) -> &[u32] {
        &REGULAR_GAME_INFO.rows_per_column
    }

    fn harbor_data(&self) -> &[HarborData] {
        &REGULAR_GAME_INFO.harbor_data
    }
}
