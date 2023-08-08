use crate::games_service::shared::game_enums::Direction;
use crate::games_service::tiles::tile_key::TileKey;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize, Copy)]
#[serde(rename_all = "PascalCase")]
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games_service::tiles::tile_key::TileKey;

    #[test]
    fn test_harbor_key_serialization() {
        let key = HarborKey::new(TileKey::new(-1, 2, 3), Direction::South);
        let tk_json = serde_json::to_string(&key).unwrap();
        let deserialized_key: HarborKey = serde_json::from_str(&tk_json).unwrap();
        assert_eq!(key, deserialized_key);
    }
}
