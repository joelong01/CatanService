use serde::{Deserialize, Serialize};

//  these are not the same as ResourceType because they have Desert and GoldMine
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq, Hash, Copy)]
pub enum TileResource {
    Back,
    Brick,
    Desert,
    GoldMine,
    Ore,
    Sheep,
    Wheat,
    Wood,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum TileOrientation {
    FaceUp,
    FaceDown,
}
