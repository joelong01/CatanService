#![allow(dead_code)]

use actix_web::HttpResponse;
/**
 * this is the module where I define the structures needed for the data in Cosmos
 */
use azure_data_cosmos::CosmosEntity;
use reqwest::StatusCode;

use serde::{Deserialize, Serialize};

use std::{env, fmt, sync::Arc};
use tokio::sync::{mpsc, RwLock};

use anyhow::{Context, Result};

use crate::games_service::{game_container::game_messages::CatanMessage, shared::game_enums::GameAction};

use super::utility::get_id;

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
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
    NoError,
    HttpError,
}
impl From<reqwest::Error> for GameError {
    fn from(err: reqwest::Error) -> Self {
        GameError::ReqwestError(format!("{:#?}", err))
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
            GameError::NoError => write!(f, "Success!"),
            GameError::HttpError => write!(f, "HttpError. see StatusCode"),
        }
    }
}

/**
 *  Every CosmosDb document needs to define the partition_key.  In Rust we do this via this trait.
 */
impl CosmosEntity for PersistUser {
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

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub struct PersistUser {
    pub id: String,                    // not set by client
    pub partition_key: u64,            // Option<> so that the client can skip this
    pub password_hash: Option<String>, // when it is pulled from Cosmos, the hash is set
    pub user_profile: UserProfile,
}

impl PersistUser {
    pub fn new() -> Self {
        Self {
            id: get_id(),
            partition_key: 1,
            password_hash: None,
            user_profile: UserProfile::default(),
        }
    }
}
impl PersistUser {
    pub fn from_client_user(client_user: &ClientUser, hash: String) -> Self {
        Self {
            id: client_user.id.clone(),
            partition_key: 1,
            password_hash: Some(hash.to_owned()),
            user_profile: client_user.user_profile.clone(),
        }
    }
    pub fn from_user_profile(profile: &UserProfile, hash: String) -> Self {
        Self {
            id: get_id(),
            partition_key: 1,
            password_hash: Some(hash.to_owned()),
            user_profile: profile.clone(),
        }
    }
}
impl Default for PersistUser {
    fn default() -> Self {
        Self::new()
    }
}
///
/// UserProfile is just information about the client.  this can be as much or little information as the app needs
/// to run
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub struct UserProfile {
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub display_name: String,
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
            email: String::default(),
            first_name: String::default(),
            last_name: String::default(),
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
            email: format!("{}@test_user.com", random_string()),
            first_name: random_name.clone(),
            last_name: random_name.clone(),
            display_name: random_name,
            picture_url: String::default(),
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

    pub fn from_persist_user(persist_user: PersistUser) -> Self {
        Self {
            id: persist_user.id,
            user_profile: persist_user.user_profile.clone(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub id: String,
    pub sub: String,
    pub exp: usize,
}
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub enum ResponseType {
    ClientUser(ClientUser),
    ClientUsers(Vec<ClientUser>),
    Token(String),
    ErrorInfo(String),
    Todo(String),
    NoData,
    ValidActions(Vec<GameAction>)
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
    pub fn to_http_response(&self) -> HttpResponse {
        let status_code = self.status;
        let response = HttpResponse::build(status_code).json(self);
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

    pub fn to_token(json: &str) -> Option<(ServiceResponse, String)> {
        let service_response: ServiceResponse = match serde_json::from_str(json) {
            Ok(sr) => sr,
            Err(_) => return None,
        };
        match service_response.response_type.clone() {
            ResponseType::Token(token) => Some((service_response, token)),
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
 *  the .devcontainer/required-secrets.json contains the list of secrets needed to run this application.  this stuctu
 *  holds them so that they are more convinient to use
 */
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ConfigEnvironmentVariables {
    pub cosmos_token: String,
    pub cosmos_account: String,
    pub ssl_key_location: String,
    pub ssl_cert_location: String,
    pub login_secret_key: String,
    pub database_name: String,
    pub container_name: String,
    pub rust_log: String,
}

impl ConfigEnvironmentVariables {
    pub fn load_from_env() -> Result<Self> {
        let cosmos_token =
            env::var("COSMOS_AUTH_TOKEN").context("COSMOS_AUTH_TOKEN not found in environment")?;
        let cosmos_account = env::var("COSMOS_ACCOUNT_NAME")
            .context("COSMOS_ACCOUNT_NAME not found in environment")?;
        let ssl_key_location =
            env::var("SSL_KEY_FILE").context("SSL_KEY_FILE not found in environment")?;
        let ssl_cert_location =
            env::var("SSL_CERT_FILE").context("SSL_CERT_FILE not found in environment")?;
        let login_secret_key =
            env::var("LOGIN_SECRET_KEY").context("LOGIN_SECRET_KEY not found in environment")?;
        let database_name = env::var("USER_DATABASE_NAME")
            .context("USER_DATABASE_NAME not found in environment")?;
        let container_name = env::var("USER_DATABASE_NAME")
            .context("USER_DATABASE_NAME not found in environment")?;
        let rust_log = env::var("RUST_LOG").context("RUST_LOG not found in environment")?;

        Ok(Self {
            cosmos_token,
            cosmos_account,
            ssl_key_location,
            ssl_cert_location,
            login_secret_key,
            database_name,
            container_name,
            rust_log,
        })
    }

    pub fn dump_values(&self) {
        log::info!("cosmos_token: {}", self.cosmos_token);
        log::info!("cosmos_account: {}", self.cosmos_account);
        log::info!("ssl_key_location: {}", self.ssl_key_location);
        log::info!("ssl_cert_location: {}", self.ssl_cert_location);
        log::info!("login_secret_key: {}", self.login_secret_key);
        log::info!("database_name: {}", self.database_name);
        log::info!("container_name: {}", self.container_name);
        log::info!("rust_log: {}", self.rust_log);
    }
}
impl Default for ConfigEnvironmentVariables {
    fn default() -> Self {
        Self {
            cosmos_token: String::new(),
            cosmos_account: String::new(),
            ssl_key_location: String::new(),
            ssl_cert_location: String::new(),
            login_secret_key: String::new(),
            database_name: "Users-Database".to_owned(),
            container_name: "User-Container".to_owned(),
            rust_log: "actix_web=trace,actix_server=trace,rust=trace".to_owned(),
        }
    }
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
