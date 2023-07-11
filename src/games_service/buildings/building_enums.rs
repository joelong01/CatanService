use serde::{Deserialize, Serialize};
use strum_macros::EnumIter;

// Enum representing the position of a building on a board
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Hash, EnumIter, Clone, Copy)]
pub enum BuildingPosition {
    Right,
    BottomRight,
    BottomLeft,
    Left,
    TopLeft,
    TopRight,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum BuildingState {
    #[serde(rename = "Empty")]
    Empty,
    #[serde(rename = "Settlement")]
    Settlement,
    #[serde(rename = "City")]
    City,
    #[serde(rename = "Pips")]
    Pips,
}
