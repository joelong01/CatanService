#![allow(dead_code)]
use crate::games_service::catan_games::game_enums::Direction;
use serde::{Deserialize, Serialize};

use super::tiles::tile_key::TileKey;

// Defining HarborType enum with variants that map to TypeScript variant strings
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum HarborType {
    #[serde(rename = "Wheat")]
    Wheat,
    #[serde(rename = "Wood")]
    Wood,
    #[serde(rename = "Ore")]
    Ore,
    #[serde(rename = "Sheep")]
    Sheep,
    #[serde(rename = "Brick")]
    Brick,
    #[serde(rename = "ThreeForOne")]
    ThreeForOne,
}
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash)]
pub struct HarborKey {
    tile_key: TileKey,
    position: Direction,
}

impl HarborKey {
    pub fn new(key: TileKey, pos: Direction) -> Self {
        Self {
            tile_key: key,
            position: pos,
        }
    }
}
// Defining HarborInfo struct to be analogous to TypeScript's class
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HarborData {
    #[serde(rename = "HarborKey")]
    key: HarborKey,
    #[serde(rename = "HarborType")]
    harbor_type: HarborType,
}

impl HarborData {
    pub fn new(key: HarborKey, harbor_type: HarborType) -> Self {
        Self { key, harbor_type }
    }
}
