// Allow dead code in this module
#![allow(dead_code)]
use super::{building_enums::BuildingState, building_key::BuildingKey};
use crate::games_service::tiles::tile_key::TileKey;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Building {
    pub building_key: BuildingKey,
    pub connected_tiles: Vec<TileKey>,
    pub aliases: Vec<BuildingKey>,
    pub pip_count: u32,
    pub owner_id: Option<String>,
    pub state: BuildingState,
}

impl Building {
    pub fn new(
        tiles: Vec<TileKey>,
        building_ids: Vec<BuildingKey>,
        pip_count: u32,
        owner_id: Option<String>,
    ) -> Self {
        Self {
            building_key: building_ids[0].clone(),
            connected_tiles: tiles,
            aliases: building_ids,
            pip_count,
            owner_id,
            state: BuildingState::Empty,
        }
    }
    pub fn default(key: BuildingKey) -> Self {
        Building {
            building_key: key,
            connected_tiles: Vec::new(),
            aliases: Vec::new(),
            pip_count: 0,
            owner_id: None,
            state: BuildingState::Empty,
        }
    }
}
