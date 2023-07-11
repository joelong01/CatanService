use crate::games_service::catan_games::game_enums::Direction;
use crate::games_service::tiles::tile_key::TileKey;
use crate::{
    shared::utility::DeserializeKeyTrait, shared::utility::SerializerKeyTrait, DeserializeKey,
    KeySerializer,
};

use serde::de::Error as SerdeError;
use serde::{ser, Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use std::fmt;

// RoadKey struct
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
}
// Use the KeySerializer macro to serialize the key
// we use this as a key to a map, and serde can only serialize keys that are strings

KeySerializer!(RoadKey {
    tile_key,
    direction
});
DeserializeKey!(RoadKey {
    tile_key,
    direction
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games_service::tiles::tile_key::TileKey;

    #[test]
    fn test_road_key_serialization() {
        let key = RoadKey::new(Direction::North, TileKey { q: -1, r: 2, s: 3 });
        let tk_json = serde_json::to_string(&key).unwrap();
        let deserialized_key: RoadKey = serde_json::from_str(&tk_json).unwrap();
        assert_eq!(key, deserialized_key);
    }
}
