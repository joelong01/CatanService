use serde::{Deserialize, Serialize};

// Defining RoadState enum with variants that map to TypeScript variant strings
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub enum RoadState {
    Unbuilt,
    Road,
    Ship,
}
