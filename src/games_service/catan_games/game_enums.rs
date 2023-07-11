#![allow(dead_code)]

use crate::shared::models::User;
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
    pub players: Vec<User>,
}
#[derive(Debug, Serialize, Clone)]
pub struct SupportedGames {
    pub catan_games: Vec<CatanGames>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum CatanGames {
    Regular,
    Expansion,
    Seafarers,
    Seafarers4Player,
}

pub enum GameType {
    Test,
    Normal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TileOrientation {
    FaceDown,
    FaceUp,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Entitlement {
    Undefined,
    DevCard,
    Settlement,
    City,
    Road,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GameState {
    Uninitialized,           // 0
    WaitingForNewGame,       // 1
    Starting,                // 2
    Dealing,                 // 3
    WaitingForStart,         // 4
    AllocateResourceForward, // 5
    AllocateResourceReverse, // 6
    DoneResourceAllocation,  // 7
    WaitingForRoll,          // 8
    Targeted,                // 9
    LostToCardsLikeMonopoly, // 10
    Supplemental,            // 11
    DoneSupplemental,        // 12
    WaitingForNext,          // 13
    LostCardsToSeven,        // 14
    MissedOpportunity,       // 15
    GamePicked,              // 16
    MustMoveBaron,           // 17
    Unknown,
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
