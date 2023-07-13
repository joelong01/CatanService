use serde::{Deserialize, Serialize};

// Defining HarborType enum with variants that map to TypeScript variant strings
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum HarborType {
    #[serde(rename = "Wheat")]
    Wheat,
    #[serde(rename = "Wood")]
    Wood,
    #[serde(rename = "Ore")]
    Ore,
    #[serde(rename = "Sheep")]
    Sheep,
    #[serde(rename = "Brick")]
    Brick,
    #[serde(rename = "ThreeForOne")]
    ThreeForOne,
}
