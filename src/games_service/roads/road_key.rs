use crate::games_service::catan_games::game_enums::Direction;
use crate::games_service::tiles::tile_key::TileKey;
use crate::{
    shared::utility::DeserializeKeyTrait, shared::utility::KeySerializerTrait, DeserializeKey,
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

// impl Serialize for RoadKey {
//     fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
//     where
//         S: Serializer,
//     {
//         let map = serde_json::json!({
//             "tile_key": self.tile_key,
//             "direction": self.direction
//         });

//         let s = serde_json::to_string(&map)
//             .map_err(ser::Error::custom)?
//             .replace("{", "[");
//         serializer.serialize_str(&s)
//     }
// }

// impl<'de> Deserialize<'de> for RoadKey {
//     fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
//     where
//         D: Deserializer<'de>,
//     {
//         let s = String::deserialize(deserializer)?;
//         let s = s.replace("[", "{");

//         let map: Value = serde_json::from_str(&s).map_err(de::Error::custom)?;

//         let tile_coord =
//             serde_json::from_value(map["tile_key"].clone()).map_err(de::Error::custom)?;
//         let direction =
//             serde_json::from_value(map["direction"].clone()).map_err(de::Error::custom)?;

//         Ok(RoadKey {
//             tile_key: tile_coord,
//             direction,
//         })
//     }
// }
