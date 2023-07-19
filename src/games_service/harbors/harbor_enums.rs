use serde::{Deserialize, Serialize};

// Defining HarborType enum with variants that map to TypeScript variant strings
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum HarborType {

    Wheat,
    Wood,
    Ore,
    Sheep,
    Brick,
    ThreeForOne,
}
