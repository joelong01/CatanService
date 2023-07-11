// Allow dead code in this module
#![allow(dead_code)]
use super::building_key::BuildingKey;
use crate::games_service::tiles::tile::Tile;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Building {
    pub key: BuildingKey,
    pub tiles: Vec<Tile>,
    pub aliases: Vec<BuildingKey>,
    pub pip_count: u32,
    pub owner_id: Option<String>,
}

impl Building {
    pub fn new(
        tiles: Vec<Tile>,
        building_ids: Vec<BuildingKey>,
        pip_count: u32,
        owner_id: Option<String>,
    ) -> Self {
        Self {
            key: building_ids[0].clone(),
            tiles,
            aliases: building_ids,
            pip_count,
            owner_id,
        }
    }
    pub fn default(key: BuildingKey) -> Self {
        Building {
            key,
            tiles: Vec::new(),
            aliases: Vec::new(),
            pip_count: 0,
            owner_id: None,
        }
    }
}
