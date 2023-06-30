use crate::shared::utility::get_id;
use azure_core::StatusCode;
/**
 * this is the module where I define the structures needed for the data in Cosmos
 */
use azure_data_cosmos::CosmosEntity;
use serde::{Deserialize, Serialize};


/**
 *  Every CosmosDb document needs to define the partition_key.  In Rust we do this via this trait.
 */
impl CosmosEntity for User {
    type Entity = u64;

    fn partition_key(&self) -> Self::Entity {
        self.partition_key
    }
}

/**
 * this is the document stored in cosmosdb.  the "id" field and the "partition_key" field are "special" in that the
 * system needs them. if id is not specified, cosmosdb will create a guild for the id (and create an 'id' field), You
 * can partition on any value, but it should be something that works well with the partion scheme that cosmos uses.
 * for this sample, we assume the db size is small, so we just partion on a number that the sample always sets to 1
 *
 */

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct User {
    pub id: String,
    pub partition_key: u64,
    pub email: String,
    pub name: String,
}

/**
 *  we are exposing a Web api to use cosmos and so a client must pass in data to create a new document.  This sample does
 *  it via form data in a POST.  it could be anything -- parameters, pass in a JSON document in the body, etc.  I picked
 *  form data because it doesn't make the URL longer, it doesn't require sharing a structure with the client, and it
 *  scales as more profile information is added (simply add more name/value pairs to the form).   actix_web will deserialize
 *  the form data to a structure, which I called PartialUser because it contains the data that the client can create,
 *  in particular it does not have the partition_key or the id
 */
#[derive(Debug, Deserialize, Serialize)]
pub struct PartialUser {
    pub email: String,
    pub name: String,
}

/**
 *  this trait makes it easy to write code to convert from a PartialUser to a User
 */

impl From<PartialUser> for User {
    fn from(client_player: PartialUser) -> Self {
        // You will generate the player_id and number here
        let id = get_id();
        let partition_key = 1;

        User {
            id,
            partition_key,
            email: client_player.email,
            name: client_player.name,
        }
    }
}

/**
 *  We want every response to be in JSON format so that it is easier to script calling the service...when
 *  we don't have "natural" JSON (e.g. when we call 'setup'), we return the JSON of this object.
 */
#[derive(Debug, Serialize, Clone)]
pub struct ServiceResponse {
    pub message: String,
    pub status: StatusCode,
    pub body: String,
}

/**
 *  the .devcontainer/required-secrets.json contains the list of secrets needed to run this application.  this stuctu
 *  holds them so that they are more convinient to use
 */
#[derive(Debug, Serialize, Deserialize)]
pub struct CosmosSecrets {
    pub token: String,
    pub account: String,
}

/**
 *  Information about a game - expect this to grow as we write code
 */
#[derive(Debug, Serialize, Clone)]
pub struct GameData {
    pub id: String,
}
#[derive(Debug, Serialize, Clone)]
pub struct SupportedGames {
    pub catan_games: Vec<CatanGames> 
}
#[allow(dead_code)]
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