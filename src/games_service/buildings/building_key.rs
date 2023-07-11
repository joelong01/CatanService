use crate::games_service::tiles::tile_key::TileKey;
use crate::{
    shared::utility::DeserializeKeyTrait, shared::utility::SerializerKeyTrait, DeserializeKey,
    KeySerializer,
};

use serde::de::Error as SerdeError;
use serde::{ser, Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use std::fmt;
use std::str::FromStr;

use super::building_enums::BuildingPosition;

// Struct representing a building alias containing position, coordinates and index of a building
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub struct BuildingKey {
    pub building_position: BuildingPosition,
    pub tile_key: TileKey,
}
// Use the KeySerializer macro to serialize the key
// we use this as a key to a map, and serde can only serialize keys that are strings

KeySerializer!(BuildingKey {
    building_position,
    tile_key
});
DeserializeKey!(BuildingKey {
    tile_key,
    building_position
});
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games_service::tiles::tile_key::TileKey;

    #[test]
    fn test_buliding_key_serialization() {
        let key = BuildingKey::new(BuildingPosition::BottomLeft, TileKey { q: -1, r: 2, s: 3 });

        let tk_json = serde_json::to_string(&key).unwrap();
        let deserialized_key: BuildingKey = serde_json::from_str(&tk_json).unwrap();
        assert_eq!(key, deserialized_key);
    }
}
