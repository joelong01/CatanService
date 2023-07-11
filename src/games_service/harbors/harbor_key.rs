use crate::games_service::catan_games::game_enums::Direction;
use crate::games_service::tiles::tile_key::TileKey;
use crate::{
    shared::utility::DeserializeKeyTrait, shared::utility::SerializerKeyTrait, DeserializeKey,
    KeySerializer,
};

use serde::de::Error as SerdeError;
use serde::{ser, Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
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

KeySerializer!(HarborKey { tile_key, position });
DeserializeKey!(HarborKey { tile_key, position });

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games_service::tiles::tile_key::TileKey;

    #[test]
    fn test_harbor_key_serialization() {
        let key = HarborKey::new (TileKey { q: -1, r: 2, s: 3 }, Direction::South);
        let tk_json = serde_json::to_string(&key).unwrap();
        let deserialized_key: HarborKey = serde_json::from_str(&tk_json).unwrap();
        assert_eq!(key, deserialized_key);
    }
}
