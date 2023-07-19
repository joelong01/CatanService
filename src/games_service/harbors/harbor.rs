#![allow(dead_code)]

use serde::{Deserialize, Serialize};

use super::{harbor_enums::HarborType, harbor_key::HarborKey};

// Defining HarborInfo struct to be analogous to TypeScript's class
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Harbor {
    pub harbor_key: HarborKey,
    pub harbor_type: HarborType,
}

impl Harbor {
    /// Constructs a new Harbor instance.
    ///
    /// This method creates a new Harbor with the given HarborKey and HarborType.
    ///
    /// # Parameters
    ///
    /// * key: HarborKey - The unique key identifying this particular harbor.
    /// * harbor_type: HarborType - The type of this harbor (e.g., specific resource harbor, generic harbor).
    ///
    /// # Returns
    ///
    /// A new Harbor instance.
    pub fn new(key: HarborKey, harbor_type: HarborType) -> Self {
        Self { harbor_key: key, harbor_type }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::games_service::{tiles::tile_key::TileKey, shared::game_enums::Direction};

    #[test]
    fn test_harbor_key_serialization() {
        let key = HarborKey::new(TileKey::new(-1, 2, 3), Direction::South);
        let tk_json = serde_json::to_string(&key).unwrap();
        let deserialized_key: HarborKey = serde_json::from_str(&tk_json).unwrap();
        assert_eq!(key, deserialized_key);
        let harbor = Harbor::new(deserialized_key, HarborType::Ore);
        let harbor_json = serde_json::to_string(&harbor).unwrap();
        let deserialized_harbor: Harbor = serde_json::from_str(&harbor_json).unwrap();
        assert_eq!(deserialized_harbor, harbor);
    }
}