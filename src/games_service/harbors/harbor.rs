#![allow(dead_code)]

use serde::{Deserialize, Serialize};

use super::{harbor_enums::HarborType, harbor_key::HarborKey};

// Defining HarborInfo struct to be analogous to TypeScript's class
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HarborData {
    #[serde(rename = "HarborKey")]
    key: HarborKey,
    #[serde(rename = "HarborType")]
    harbor_type: HarborType,
}

impl HarborData {
    pub fn new(key: HarborKey, harbor_type: HarborType) -> Self {
        Self { key, harbor_type }
    }
}
