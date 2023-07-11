use serde::{Deserialize, Serialize};

// Defining RoadState enum with variants that map to TypeScript variant strings
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum RoadState {
    #[serde(rename = "unbuilt")]
    Unbuilt,
    #[serde(rename = "road")]
    Road,
    #[serde(rename = "ship")]
    Ship,
}
