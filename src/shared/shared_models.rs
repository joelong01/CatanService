#![allow(dead_code)]

use actix_web::HttpResponse;

/**
 * this is the module where I define the structures needed for the data in Cosmos
 */
use reqwest::StatusCode;

use serde::{Deserialize, Serialize};
use strum_macros::Display;


use std::{fmt, fmt::Display, fmt::Formatter, sync::Arc};
use tokio::sync::{mpsc, RwLock};

use anyhow::Result;

use crate::games_service::{
    catan_games::games::regular::regular_game::RegularGame,
    game_container::game_messages::CatanMessage,
    shared::game_enums::{CatanGames, GameAction},
};


use super::service_models::PersistUser;

//
//  this also supports Eq, PartialEq, Clone, Serialize, and Deserialize via custom implementation
#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Clone)]
pub enum GameError {
    MissingData(String),
    BadActionData(String),
    BadId(String),
    ChannelError(String),
    AlreadyExists(String),
    ActionError(String),
    TooFewPlayers(usize),
    TooManyPlayers(usize),
    ReqwestError(String),
    NoError(String),
    #[serde(serialize_with = "serialize_status_code")]
    #[serde(deserialize_with = "deserialize_status_code")]
    HttpError(reqwest::StatusCode),
    AzError(String),
    SerdeError(String),
    AzureCoreError(String),
}

// we need a From<> for each error type we add to use the error propagation ?
impl From<reqwest::Error> for GameError {
    fn from(err: reqwest::Error) -> Self {
        GameError::ReqwestError(format!("{:#?}", err))
    }
}

impl From<serde_json::Error> for GameError {
    fn from(err: serde_json::Error) -> Self {
        GameError::SerdeError(err.to_string())
    }
}

impl From<azure_core::Error> for GameError {
    fn from(err: azure_core::Error) -> Self {
        GameError::AzureCoreError(err.to_string())
    }
}

impl From<std::io::Error> for GameError {
    fn from(err: std::io::Error) -> Self {
        GameError::AzError(format!("{:#?}", err))
    }
}

impl fmt::Display for GameError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            GameError::ChannelError(desc) => {
                write!(f, "Error reading tokio channel (ChannelError): {}", desc)
            }
            GameError::AlreadyExists(desc) => {
                write!(f, "Resource Already Exists (AlreadyExists): {}", desc)
            }
            GameError::ActionError(desc) => write!(f, "Action Error: {}", desc),
            GameError::MissingData(desc) => write!(f, "Missing Data {}", desc),
            GameError::BadActionData(desc) => write!(f, "Bad Data {}", desc),
            GameError::BadId(desc) => write!(f, "Bad Id {}", desc),
            GameError::TooFewPlayers(s) => write!(f, "Min Players {}", s),
            GameError::TooManyPlayers(c) => write!(f, "Max Players {}", c),
            GameError::ReqwestError(c) => write!(f, "ReqwestError error: {}", c),
            GameError::NoError(s) => write!(f, "Success!: {}", s),
            GameError::HttpError(code) => write!(f, "HttpError. {:#?}", code),
            GameError::AzError(e) => write!(f, "AzError: {:#?}", e),
            GameError::SerdeError(e) => write!(f, "Serde Error: {:#?}", e),
            GameError::AzureCoreError(e) => write!(f, "Azure Core error: {:#?}", e),
        }
    }
}

///
/// Connected users are must be actively connected to the system and particpate in long_polling
/// LocalUsers do not, and instead get messages on the creators thread.  Only local users for the creater
/// should be shown by the client
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum UserType {
    Connected,
    Local,
}

///
/// UserProfile is just information about the client.  this can be as much or little information as the app needs
/// to run
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub struct UserProfile {
    pub user_type: UserType,
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub display_name: String,
    pub phone_number: String,
    pub picture_url: String,
    pub foreground_color: String,
    pub background_color: String,
    pub text_color: String,
    pub games_played: Option<u16>,
    pub games_won: Option<u16>,
}
impl Default for UserProfile {
    fn default() -> Self {
        UserProfile {
            user_type: UserType::Connected,
            email: String::default(),
            first_name: String::default(),
            last_name: String::default(),
            phone_number: String::default(),
            display_name: String::default(),
            picture_url: String::default(),
            foreground_color: String::default(),
            background_color: String::default(),
            text_color: String::default(),
            games_played: None,
            games_won: None,
        }
    }
}

impl UserProfile {
    pub fn is_equal_byval(&self, other: &UserProfile) -> bool {
        self.email == other.email
            && self.first_name == other.first_name
            && self.last_name == other.last_name
            && self.display_name == other.display_name
            && self.phone_number == other.phone_number
            && self.picture_url == other.picture_url
            && self.foreground_color == other.foreground_color
            && self.background_color == other.background_color
            && self.text_color == other.text_color
            && self.games_played.unwrap_or(0) == other.games_played.unwrap_or(0)
            && self.games_won.unwrap_or(0) == other.games_won.unwrap_or(0)
    }

    pub fn new_test_user() -> Self {
        let random_string = || {
            use rand::distributions::Alphanumeric;
            use rand::{thread_rng, Rng};
            thread_rng()
                .sample_iter(&Alphanumeric)
                .take(8)
                .map(char::from)
                .collect::<String>()
        };

        let random_name = random_string();
        UserProfile {
            user_type: UserType::Connected,
            email: format!("{}@test.com", random_string()),
            first_name: random_name.clone(),
            last_name: random_name.clone(),
            display_name: random_name,
            picture_url: String::default(),
            phone_number: String::default(),
            foreground_color: String::default(),
            background_color: String::default(),
            text_color: String::default(),
            games_played: None,
            games_won: None,
        }
    }
}
///
/// This is the struct that is returned to the clien whenever User data needs to be returned.  it is also the format
/// that data is passed from the client to the service.  Note that the password is not in this structure -- it passes
/// from the client in a header, as does the JWT token when it is needed.
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub struct ClientUser {
    pub id: String,
    pub user_profile: UserProfile,
}

impl ClientUser {
    /// Creates a new [`ClientUser`].
    fn new(id: String) -> Self {
        Self {
            id,
            user_profile: UserProfile::default(),
        }
    }

    pub fn from_persist_user(persist_user: &PersistUser) -> Self {
        Self {
            id: persist_user.id.clone(),
            user_profile: persist_user.user_profile.clone(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq, Display)]
pub enum ResponseType {
    ClientUser(ClientUser),
    ClientUsers(Vec<ClientUser>),
    Token(String),
    Url(String),
    ErrorInfo(String),
    Todo(String),
    NoData,
    ValidActions(Vec<GameAction>),
    Game(RegularGame),
    SupportedGames(Vec<CatanGames>),
    SendMessageError(Vec<(String, GameError)>),
    ServiceMessage(CatanMessage),
    AzError(String),
    SerdeError(String),
}

/**
 *  We want every response to be in JSON format so that it is easier to script calling the service.
 */
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub struct ServiceResponse {
    pub message: String,
    #[serde(serialize_with = "serialize_status_code")]
    #[serde(deserialize_with = "deserialize_status_code")]
    pub status: reqwest::StatusCode,
    pub response_type: ResponseType,
    pub game_error: GameError,
}
impl Display for ServiceResponse {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        // Convert the struct to a pretty-printed JSON string.
        let json = serde_json::to_string_pretty(self).map_err(|_| fmt::Error)?;
        write!(f, "{}", json)
    }
}

impl From<serde_json::Error> for ServiceResponse {
    fn from(err: serde_json::Error) -> Self {
        ServiceResponse::new(
            "Unable to deserialize response",
            StatusCode::INTERNAL_SERVER_ERROR,
            ResponseType::NoData,
            GameError::SerdeError(err.to_string()),
        )
    }
}



impl ServiceResponse {
    pub fn new(
        message: &str,
        status: StatusCode,
        response_type: ResponseType,
        error: GameError,
    ) -> Self {
        ServiceResponse {
            message: message.into(),
            status,
            response_type,
            game_error: error,
        }
    }

    pub fn new_generic_ok(msg: &str) -> Self {
        ServiceResponse {
            message: msg.to_owned(),
            status: StatusCode::OK,
            response_type: ResponseType::NoData,
            game_error: GameError::NoError(String::default()),
        }
    }

    pub fn assert_success(&self, msg: &str) -> &Self {
        if !self.status.is_success() {
            panic!("{}", msg.to_string());
        }

        self
    }

    pub fn new_bad_id(msg: &str, id: &str) -> Self {
        ServiceResponse {
            message: msg.to_owned(),
            status: StatusCode::BAD_REQUEST,
            response_type: ResponseType::NoData,
            game_error: GameError::BadId(id.to_owned()),
        }
    }

    pub fn to_http_response(&self) -> HttpResponse {
        let serialized = serde_json::to_string(self).expect("Failed to serialize ServiceResponse");

        let response = HttpResponse::build(self.status).body(serialized);
        response
    }

    pub fn get_client_user(&self) -> Option<ClientUser> {
        match self.response_type.clone() {
            ResponseType::ClientUser(data) => Some(data),
            _ => None,
        }
    }

    pub fn to_client_user(json: &str) -> Option<(ServiceResponse, ClientUser)> {
        let service_response: ServiceResponse = match serde_json::from_str(json) {
            Ok(sr) => sr,
            Err(_) => return None,
        };
        match service_response.response_type.clone() {
            ResponseType::ClientUser(client_user) => Some((service_response, client_user)),
            _ => None,
        }
    }

    pub fn to_client_users(json: &str) -> Option<(ServiceResponse, Vec<ClientUser>)> {
        let service_response: ServiceResponse = match serde_json::from_str(json) {
            Ok(sr) => sr,
            Err(_) => return None,
        };
        match service_response.response_type.clone() {
            ResponseType::ClientUsers(client_users) => Some((service_response, client_users)),
            _ => None,
        }
    }

    pub fn json_to_token(json: &str) -> Option<(ServiceResponse, String)> {
        let service_response: ServiceResponse = match serde_json::from_str(json) {
            Ok(sr) => sr,
            Err(_) => return None,
        };
        match service_response.get_token() {
            Some(token) => Some((service_response, token)),
            None => None,
        }
    }
    pub fn get_token(&self) -> Option<String> {
        // Extract auth token from response
        match &self.response_type {
            ResponseType::Token(token) => Some(token.clone()),
            _ => None,
        }
    }
    pub fn get_url(&self) -> Option<String> {
        // Extract auth token from response
        match &self.response_type {
            ResponseType::Url(url) => Some(url.clone()),
            _ => None,
        }
    }
    pub fn get_game(&self) -> Option<RegularGame> {
        match &self.response_type {
            ResponseType::Game(game) => Some(game.clone()),
            _ => None,
        }
    }

    pub fn get_client_users(&self) -> Option<Vec<ClientUser>> {
        match &self.response_type {
            ResponseType::ClientUsers(users) => Some(users.clone()),
            _ => None,
        }
    }
    pub fn get_actions(&self) -> Option<Vec<GameAction>> {
        match &self.response_type {
            ResponseType::ValidActions(actions) => Some(actions.clone()),
            _ => None,
        }
    }
    pub fn get_service_message(&self) -> Option<CatanMessage> {
        match &self.response_type {
            ResponseType::ServiceMessage(msg) => Some(msg.clone()),
            _ => None,
        }
    }

    pub fn to_error_info(json: &str) -> Option<(ServiceResponse, String)> {
        let service_response: ServiceResponse = match serde_json::from_str(json) {
            Ok(sr) => sr,
            Err(_) => return None,
        };
        match service_response.response_type.clone() {
            ResponseType::ErrorInfo(error_info) => Some((service_response, error_info)),
            _ => None,
        }
    }

}
fn serialize_status_code<S>(status: &reqwest::StatusCode, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_u16(status.as_u16())
}

fn deserialize_status_code<'de, D>(deserializer: D) -> Result<reqwest::StatusCode, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let code = u16::deserialize(deserializer)?;
    Ok(StatusCode::from_u16(code).map_err(serde::de::Error::custom)?)
}



/**
 * hold the data that both the Lobby and the GameContainer use to keep track of the waiting clients
 */
#[derive(Debug)]
pub struct LongPollUser {
    pub user_id: String,
    pub name: String,
    pub tx: mpsc::Sender<CatanMessage>,
    pub rx: Arc<RwLock<mpsc::Receiver<CatanMessage>>>,
}

impl LongPollUser {
    pub fn new(user_id: &str, name: &str) -> Self {
        let (tx, rx) = mpsc::channel(0x64);
        Self {
            user_id: user_id.into(),
            name: name.into(),
            rx: Arc::new(RwLock::new(rx)),
            tx,
        }
    }
}
