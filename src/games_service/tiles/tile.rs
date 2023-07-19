#![allow(dead_code)]
use crate::games_service::buildings::{building::Building, building_enums::BuildingPosition};
use crate::games_service::shared::game_enums::Direction;
use crate::games_service::roads::road::Road;

use super::tile_enums::TileResource;
use super::tile_key::TileKey;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use std::collections::HashMap;

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Tile {
    pub current_resource: TileResource, // the resource (including a temp resource) that the tile currently holds
    pub original_resource: TileResource, // the orginal resource the tile started with
    pub roll: u32,                      // the Catan Number that the tile should display
    pub tile_key: TileKey,                   // the position of the tile on the board

    #[serde_as(as = "Vec<(_, _)>")]
    pub roads: HashMap<Direction, Road>, // all the roads around the tile

    #[serde_as(as = "Vec<(_, _)>")]
    pub owned_buildings: HashMap<BuildingPosition, Building>, // the owned buildings that get resources for this tile
}
impl Tile {
    pub fn new(key: TileKey, roll: u32, resource: TileResource) -> Self {
        Self {
            tile_key: key,
            roll,
            current_resource: resource.clone(),
            original_resource: resource.clone(),
            roads: HashMap::new(),
            owned_buildings: HashMap::new(),
        }
    }
}
//
