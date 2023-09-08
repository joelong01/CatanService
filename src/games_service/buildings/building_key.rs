#![allow(dead_code)]
use crate::games_service::shared::game_enums::Direction;
use crate::games_service::tiles::tile::Tile;
use crate::games_service::tiles::tile_key::TileKey;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

use super::building_enums::BuildingPosition;

// Struct representing a building alias containing position, coordinates and index of a building
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct BuildingKey {
    pub building_position: BuildingPosition,
    pub tile_key: TileKey,
}

impl BuildingKey {
    /// Constructs a new instance of `BuildingKey` with the given `BuildingPosition` and `TileKey`.
    ///
    /// The `new` function is typically used to create a `BuildingKey` when the building's position
    /// on the tile and the tile's key are known.
    ///
    /// # Arguments
    ///
    /// * `building_position` - A `BuildingPosition` that represents the building's position on a tile.
    /// * `tile_key` - A `TileKey` that uniquely identifies a tile.
    ///
    /// # Returns
    ///
    /// * `BuildingKey` - A new instance of `BuildingKey`.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let building_key = BuildingKey::new(BuildingPosition::TopRight, tile_key);
    /// ```
    ///
    pub fn new(building_position: BuildingPosition, tile_key: TileKey) -> Self {
        Self {
            building_position,
            tile_key,
        }
    }

    /// Returns a `Vec` of `BuildingKey`s that represent buildings adjacent to the current building.
    ///
    /// The function determines the adjacent buildings based on the current building's position
    /// and the layout of the provided tiles. It utilizes the `get_neighbor_key` method
    /// to find the neighboring tiles and generates corresponding `BuildingKey`s.
    /// Buildings on tiles that are not included in the provided `tiles` HashMap are excluded.
    ///
    /// # Arguments
    ///
    /// * `&self` - A reference to the current instance of `BuildingKey`.
    /// * `tiles` - A reference to a HashMap holding `TileKey`-`Tile` pairs.
    ///
    /// # Returns
    ///
    /// * `Vec<BuildingKey>` - A vector containing `BuildingKey`s for each adjacent building.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let adjacent_keys = building_key.get_adjacent_building_keys(&tiles);
    /// ```
    ///
    pub fn get_adjacent_building_keys(&self, tiles: &HashMap<TileKey, Tile>) -> Vec<BuildingKey> {
        let directions: Vec<(BuildingPosition, Direction)> = match self.building_position {
            BuildingPosition::TopRight => vec![
                (BuildingPosition::BottomRight, Direction::North),
                (BuildingPosition::Left, Direction::NorthEast),
            ],
            BuildingPosition::Right => vec![
                (BuildingPosition::BottomLeft, Direction::NorthEast),
                (BuildingPosition::TopLeft, Direction::SouthEast),
            ],
            BuildingPosition::BottomRight => vec![
                (BuildingPosition::TopRight, Direction::South),
                (BuildingPosition::Left, Direction::SouthEast),
            ],
            BuildingPosition::BottomLeft => vec![
                (BuildingPosition::Right, Direction::SouthWest),
                (BuildingPosition::TopLeft, Direction::South),
            ],
            BuildingPosition::Left => vec![
                (BuildingPosition::BottomRight, Direction::NorthWest),
                (BuildingPosition::TopRight, Direction::SouthWest),
            ],
            BuildingPosition::TopLeft => vec![
                (BuildingPosition::Right, Direction::NorthWest),
                (BuildingPosition::BottomLeft, Direction::North),
            ],
        };

        let adjacent_keys = directions
            .iter()
            .filter_map(|(building_pos, dir)| {
                let neighbor_key = self.tile_key.get_neighbor_key(*dir);
                if tiles.contains_key(&neighbor_key) {
                    Some(BuildingKey::new(*building_pos, neighbor_key))
                } else {
                    None
                }
            })
            .collect();

        adjacent_keys
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games_service::tiles::tile_key::TileKey;

    #[test]
    fn test_buliding_key_serialization() {
        let key = BuildingKey::new(BuildingPosition::BottomLeft, TileKey::new(-1, 2, 3));

        let tk_json = serde_json::to_string(&key).unwrap();
        let deserialized_key: BuildingKey = serde_json::from_str(&tk_json).unwrap();
        assert_eq!(key, deserialized_key);
    }
}
