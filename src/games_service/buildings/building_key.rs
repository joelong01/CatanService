use crate::games_service::tiles::tile_key::TileKey;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

use super::building_enums::BuildingPosition;

// Struct representing a building alias containing position, coordinates and index of a building
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub struct BuildingKey {
    #[serde(rename = "position")]
    pub building_position: BuildingPosition,
    #[serde(rename = "coordinates")]
    pub tile_key: TileKey,
}

impl BuildingKey {
    // Function to create a new instance of BuildingAlias
    pub fn new(building_position: BuildingPosition, tile_key: TileKey) -> Self {
        Self {
            building_position,
            tile_key,
        }
    }
}

// Implementing the FromStr trait for BuildingAlias, to convert a string into a BuildingAlias
impl FromStr for BuildingKey {
    type Err = serde_json::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Use serde_json to perform the conversion
        serde_json::from_str(s)
    }
}

// Implementing Display trait for BuildingAlias, to convert a BuildingAlias into a string
impl fmt::Display for BuildingKey {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // Use serde_json to convert the BuildingAlias into a JSON string
        write!(
            f,
            "{}",
            serde_json::to_string(self).map_err(|_| fmt::Error)?
        )
    }
}
