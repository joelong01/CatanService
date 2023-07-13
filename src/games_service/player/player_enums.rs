use serde::{Deserialize, Serialize};
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub enum Weapon {
    Knight,
    RolledSeven,
    PirateShip,
}
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct Target {
    weapon: Weapon,
    target: String, // the user ID of the target
}
