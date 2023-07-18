#![allow(dead_code)]
use crate::games_service::shared::game_enums::Direction;
use crate::games_service::tiles::tile_key::TileKey;
use serde::{Deserialize, Serialize};
use std::fmt;

// RoadKey struct
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoadKey {
    tile_key: TileKey,    // the tile coordinates that Direction is relative to
    direction: Direction, // the direction that represents the side of the tile that the road is on
}

impl RoadKey {
    pub fn new(direction: Direction, tile: TileKey) -> Self {
        Self {
            direction,
            tile_key: tile,
        }
    }
}

impl fmt::Display for RoadKey {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}|{}",
            self.direction as i32,
            serde_json::to_string(&self.tile_key).map_err(|_| fmt::Error)?
        )
    }
}#
[cfg(test)]
mod tests {
    use super::*;
    use crate::games_service::tiles::tile_key::TileKey;

    #[test]
    fn test_road_key_serialization() {
        let key = RoadKey::new(Direction::North, TileKey::new(-1, 2, 3));
        let tk_json = serde_json::to_string(&key).unwrap();
        print!("{}", tk_json);
        let deserialized_key: RoadKey = serde_json::from_str(&tk_json).unwrap();

        assert_eq!(key, deserialized_key);
    }
}
