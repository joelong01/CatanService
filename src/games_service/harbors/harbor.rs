#![allow(dead_code)]

use serde::{Deserialize, Serialize};

use super::{harbor_enums::HarborType, harbor_key::HarborKey};

// Defining HarborInfo struct to be analogous to TypeScript's class
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Harbor {
    #[serde(rename = "HarborKey")]
    pub key: HarborKey,
    #[serde(rename = "HarborType")]
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
        Self { key, harbor_type }
    }
}
