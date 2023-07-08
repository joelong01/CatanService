/**
 *  Information about a game - expect this to grow as we write code
 */
use crate::shared::models::User;

#[derive(Debug, Serialize, Clone)]
pub struct GameData {
    pub id: String,
    pub players: Vec<User>,
}
#[derive(Debug, Serialize, Clone)]
pub struct SupportedGames {
    pub catan_games: Vec<CatanGames>,
}
#[allow(dead_code)]
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum CatanGames {
    Regular,
    Expansion,
    Seafarers,
    Seafarers4Player,
}
#[allow(dead_code)]
pub enum GameType {
    Test,
    Normal,
}
#[allow(dead_code)]
pub enum TileOrientation {
    FaceDown,
    FaceUp,
    None,
}
#[allow(dead_code)]
pub enum HarborType {
    Sheep,
    Wood,
    Ore,
    Wheat,
    Brick,
    ThreeForOne,
    Uninitialized,
    None,
}
#[allow(dead_code)]
pub enum Entitlement {
    Undefined,
    DevCard,
    Settlement,
    City,
    Road,
}
#[allow(dead_code)]
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
#[allow(dead_code)]
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
#[allow(dead_code)]
pub enum DevCardType {
    Knight,
    VictoryPoint,
    YearOfPlenty,
    RoadBuilding,
    Monopoly,
    Unknown,
    Back,
}
#[allow(dead_code)]
pub enum BodyType {
    TradeResources,
    None,
    GameInfo,
    TradeResourcesList,
}
