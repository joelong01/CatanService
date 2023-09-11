#![allow(dead_code)]

use crate::shared::shared_models::ClientUser;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::{fmt, str::FromStr};
use strum_macros::EnumIter;
/**
 *  Information about a game - expect this to grow as we write code
 */
#[derive(Debug, Serialize, Clone, Deserialize)]
pub struct GameData {
    pub id: String,
    pub players: Vec<ClientUser>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SupportedGames {
    pub catan_games: Vec<CatanGames>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Copy, Eq)]
pub enum CatanGames {
    Regular,
    Expansion,
    Seafarers,
    Seafarers4Player,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Copy)]
pub enum GameType {
    Test,
    Normal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Copy)]
pub enum TileOrientation {
    FaceDown,
    FaceUp,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Copy)]
pub enum Entitlement {
    Undefined,
    DevCard,
    Settlement,
    City,
    Road,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum GameAction {
    AddPlayer,
    NewBoard,
    SetOrder,
    Start,
    Buy,
    Build,
    Roll,
    MoveBaron,
    Trade,
    Next,
    Undo,
    Redo,
}

//
//  answers the question "what are we doing now?"
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Copy)]
#[serde(rename_all = "PascalCase")]
pub enum GameState {
    AddingPlayers,
    ChoosingBoard,
    SettingPlayerOrder,
    AllocateResourceForward,
    AllocateResourceReverse,
    WaitingForRoll,
    MustMoveBaron,
    BuyingAndTrading,
    Supplemental,
    GameOver,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum GamePhase {
    SettingUp,
    Playing,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResourceType {
    Sheep,
    Wood,
    Ore,
    Wheat,
    Brick,
    GoldMine,
    Desert,
    Back,
    None,
    Sea,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DevCardType {
    Knight,
    VictoryPoint,
    YearOfPlenty,
    RoadBuilding,
    Monopoly,
    Unknown,
    Back,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BodyType {
    TradeResources,
    None,
    GameInfo,
    TradeResourcesList,
}
/**
 * Direction support
 */

// custom parsing error:
#[derive(Debug)]
pub enum DirectionError {
    BadDirection,
}

impl fmt::Display for DirectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Invalid direction")
    }
}
impl Error for DirectionError {}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Copy, EnumIter)]
pub enum Direction {
    North,
    NorthEast,
    SouthEast,
    South,
    SouthWest,
    NorthWest,
}

impl FromStr for Direction {
    type Err = DirectionError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.parse::<i32>() {
            Ok(0) => Ok(Direction::North),
            Ok(1) => Ok(Direction::NorthEast),
            Ok(2) => Ok(Direction::SouthEast),
            Ok(3) => Ok(Direction::South),
            Ok(4) => Ok(Direction::SouthWest),
            Ok(5) => Ok(Direction::NorthWest),
            _ => Err(DirectionError::BadDirection),
        }
    }
}
