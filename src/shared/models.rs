#![allow(dead_code)]

use actix_web::HttpResponse;
/**
 * this is the module where I define the structures needed for the data in Cosmos
 */
use azure_data_cosmos::CosmosEntity;
use reqwest::StatusCode;
use serde::de::{self, Deserializer, Visitor};
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};
use strum_macros::Display;

use std::collections::HashMap;
use std::{
    env, fmt,
    fmt::Display,
    fmt::Formatter,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::{mpsc, RwLock};

use anyhow::Result;

use crate::macros::convert_status_code;
use crate::{
    games_service::{
        catan_games::games::regular::regular_game::RegularGame,
        game_container::game_messages::CatanMessage,
        shared::game_enums::{CatanGames, GameAction},
    },
    middleware::environment_mw::TestContext,
};

use super::utility::get_id;

#[derive(Debug)]
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
    HttpError(reqwest::StatusCode),
    AzError(String),
    SerdeError(serde_json::Error),
    AzureCoreError(azure_core::Error),
}
impl PartialEq for GameError {
    fn eq(&self, other: &Self) -> bool {
        use GameError::*;
        match (self, other) {
            (MissingData(a), MissingData(b)) => a == b,
            (BadActionData(a), BadActionData(b)) => a == b,
            (SerdeError(a), SerdeError(b)) => a.to_string() == b.to_string(),

            (BadId(a), BadId(b)) => a == b,

            (ChannelError(a), ChannelError(b)) => a == b,

            (AlreadyExists(a), AlreadyExists(b)) => a == b,

            (ActionError(a), ActionError(b)) => a == b,

            (TooFewPlayers(a), TooFewPlayers(b)) => a == b,

            (TooManyPlayers(a), TooManyPlayers(b)) => a == b,

            (ReqwestError(a), ReqwestError(b)) => a == b,

            (HttpError(a), HttpError(b)) => a == b,

            (AzError(a), AzError(b)) => a == b,

            (AzureCoreError(a), AzureCoreError(b)) => {
                format!("{:#?}", a) == format!("{:#?}", b) // this is likely not a good idea...
            }
            _ => false,
        }
    }
}
impl Clone for GameError {
    fn clone(&self) -> Self {
        match self {
            GameError::MissingData(s) => GameError::MissingData(s.clone()),
            GameError::BadActionData(s) => GameError::BadActionData(s.clone()),
            GameError::BadId(s) => GameError::BadId(s.clone()),
            GameError::ChannelError(s) => GameError::ChannelError(s.clone()),
            GameError::AlreadyExists(s) => GameError::AlreadyExists(s.clone()),
            GameError::ActionError(s) => GameError::ActionError(s.clone()),
            GameError::TooFewPlayers(n) => GameError::TooFewPlayers(*n),
            GameError::TooManyPlayers(n) => GameError::TooManyPlayers(*n),
            GameError::ReqwestError(s) => GameError::ReqwestError(s.clone()),
            GameError::NoError(s) => GameError::NoError(s.clone()),
            GameError::HttpError(status) => GameError::HttpError(*status),
            GameError::AzError(s) => GameError::AzError(s.clone()),
            GameError::SerdeError(err) => {
                // Example of converting serde error to string and back.
                // This is lossy!
                let err_str = err.to_string();
                GameError::SerdeError(
                    serde_json::from_str::<serde_json::Value>(&err_str).unwrap_err(),
                )
            }
            GameError::AzureCoreError(_) => {
                // Here you might need a similar approach as with the serde error
                // if azure_core::Error doesn't have a direct clone mechanism.
                // For the purpose of this example, we'll just panic.
                panic!("AzureCoreError cannot be cloned directly")
            }
        }
    }
}

impl Eq for GameError {}

// we need a From<> for each error type we add to use the error propagation ?
impl From<reqwest::Error> for GameError {
    fn from(err: reqwest::Error) -> Self {
        GameError::ReqwestError(format!("{:#?}", err))
    }
}

impl From<serde_json::Error> for GameError {
    fn from(err: serde_json::Error) -> Self {
        GameError::SerdeError(err)
    }
}

impl From<azure_core::Error> for GameError {
    fn from(err: azure_core::Error) -> Self {
        GameError::AzureCoreError(err)
    }
}

impl From<std::io::Error> for GameError {
    fn from(err: std::io::Error) -> Self {
        GameError::AzError(format!("{:#?}", err))
    }
}

impl std::error::Error for GameError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            GameError::SerdeError(e) => Some(e),
            GameError::AzureCoreError(e) => Some(e),

            _ => None,
        }
    }
}

impl Serialize for GameError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            GameError::HttpError(status_code) => serializer.serialize_u16(status_code.as_u16()),
            GameError::SerdeError(e) => serializer.serialize_str(&format!("{:#?}", e)),
            GameError::AzureCoreError(e) => serializer.serialize_str(&format!("{:#?}", e)),
            GameError::MissingData(msg)
            | GameError::BadActionData(msg)
            | GameError::BadId(msg)
            | GameError::ChannelError(msg)
            | GameError::AlreadyExists(msg)
            | GameError::ActionError(msg)
            | GameError::NoError(msg)
            | GameError::AzError(msg) => serializer.serialize_str(msg),
            GameError::TooFewPlayers(count) | GameError::TooManyPlayers(count) => {
                serializer.serialize_u64(*count as u64)
            }
            GameError::ReqwestError(r_err) => serializer.serialize_str(r_err),
        }
    }
}

impl<'de> Deserialize<'de> for GameError {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct GameErrorVisitor;

        impl<'de> Visitor<'de> for GameErrorVisitor {
            type Value = GameError;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a valid GameError representation")
            }

            fn visit_u16<E>(self, value: u16) -> Result<GameError, E>
            where
                E: de::Error,
            {
                // If we receive a u16, assume it's an HTTP status code.
                Ok(GameError::HttpError(
                    reqwest::StatusCode::from_u16(value).map_err(de::Error::custom)?,
                ))
            }
        }

        // Use the visitor for deserialization
        deserializer.deserialize_any(GameErrorVisitor)
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
 * system needs them. if id is not specified, cosmosdb will create a guid for the id (and create an 'id' field), You
 * can partition on any value, but it should be something that works well with the partion scheme that cosmos uses.
 * for this sample, we assume the db size is small, so we just partion on a number that the sample always sets to 1
 * note:  you don't want to use camelCase or PascalCase for this as you need to be careful about how 'id' and 'partionKey'
 * are spelled and capitalized
 */

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]

pub struct PersistUser {
    pub id: String, // not set by client
    #[serde(rename = "partitionKey")]
    pub partition_key: u64, // the cosmos client seems to care about the spelling of both id and partitionKey
    pub password_hash: Option<String>, // when it is pulled from Cosmos, the hash is set
    pub validated_email: bool,         // has the mail been validated?
    pub validated_phone: bool,         // has the phone number been validated?
    pub user_profile: UserProfile,
    pub phone_code: Option<String>,
}

impl PersistUser {
    pub fn new() -> Self {
        Self {
            id: get_id(),
            partition_key: 1,
            password_hash: None,
            user_profile: UserProfile::default(),
            validated_email: false,
            validated_phone: false,
            phone_code: None,
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
            validated_email: false,
            validated_phone: false,
            phone_code: None,
        }
    }
    pub fn from_user_profile(profile: &UserProfile, hash: String) -> Self {
        Self {
            id: get_id(),
            partition_key: 1,
            password_hash: Some(hash.clone()),
            user_profile: profile.clone(),
            validated_email: false,
            validated_phone: false,
            phone_code: None,
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
// DO NOT ADD A #[serde(rename_all = "PascalCase")] macro to this struct!
// it will throw an error and you'll spend hours figuring out why it doesn't work - the rust bcrypt library cares about
// capitalization and enforces standard claim names
#[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct Claims {
    pub id: String,
    pub sub: String,
    pub exp: usize,
    pub test_context: Option<TestContext>,
}

impl Claims {
    pub fn new(
        id: &str,
        email: &str,
        duration_secs: u64,
        test_context: &Option<TestContext>,
    ) -> Self {
        let exp = ((SystemTime::now() + Duration::from_secs(duration_secs))
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()) as usize;
        Self {
            id: id.to_owned(),
            sub: email.to_owned(),
            exp,
            test_context: test_context.clone(),
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

impl From<GameError> for ServiceResponse {
    fn from(err: GameError) -> Self {
        let (message, status, response_type) = match &err {
            GameError::SerdeError(e) => (
                format!("Serde error: {}", e),
                reqwest::StatusCode::BAD_REQUEST,
                ResponseType::ErrorInfo(format!("Serde error: {:#?}", e)),
            ),
            GameError::AzureCoreError(e) => {
                let status_code = match e.as_http_error() {
                    Some(http_err) => convert_status_code(http_err.status()),
                    None => reqwest::StatusCode::INTERNAL_SERVER_ERROR,
                };

                (
                    format!("Azure error: {}", e),
                    status_code,
                    ResponseType::ErrorInfo(format!("Azure error: {:#?}", e)),
                )
            }
            GameError::ReqwestError(e) => {
                let error_msg = e.clone();
                (
                    error_msg.clone(),
                    reqwest::StatusCode::INTERNAL_SERVER_ERROR,
                    ResponseType::ErrorInfo(error_msg),
                )
            }
            // ... Add handling for other error variants...
            _ => (
                format!("Unknown error"),
                reqwest::StatusCode::INTERNAL_SERVER_ERROR,
                ResponseType::ErrorInfo("Unknown error".into()),
            ),
        };

        ServiceResponse {
            message,
            status,
            response_type,
            game_error: err,
        }
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
 *  the .devcontainer/required-secrets.json contains the list of secrets needed to run this application.  this stuctu
 *  holds them so that they are more convinient to use
 */
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ConfigEnvironmentVariables {
    pub resource_group: String,
    pub kv_name: String,
    pub azure_location: String,

    pub cosmos_token: String,
    pub cosmos_account: String,
    pub cosmos_database_name: String,
    pub cosmos_collections: Vec<String>,

    pub ssl_key_location: String,
    pub ssl_cert_location: String,
    pub login_secret_key: String,
    pub validation_secret_key: String,

    pub rust_log: String,

    pub test_phone_number: String,
    pub service_phone_number: String,

    pub test_email: String,
    pub service_email: String,

    pub name_value_map: HashMap<String, String>,
}
fn insert_env_to_map(name_map: &mut HashMap<String, String>, env_var_name: &str) -> Result<String> {
    let value = env::var(env_var_name).expect(&format!(
        "{} not found in environment - unable to continue",
        env_var_name
    ));
    name_map.insert(value.clone(), format!("${}", env_var_name));
    Ok(value)
}
impl ConfigEnvironmentVariables {
    pub fn load_from_env() -> Result<Self> {
        let mut name_map = HashMap::new();

        let resource_group = insert_env_to_map(&mut name_map, "AZURE_RESOURCE_GROUP")?;
        let kv_name = insert_env_to_map(&mut name_map, "KEV_VAULT_NAME")?;
        let cosmos_token = insert_env_to_map(&mut name_map, "COSMOS_AUTH_TOKEN")?;
        let cosmos_account = insert_env_to_map(&mut name_map, "COSMOS_ACCOUNT_NAME")?;
        let cosmos_database = insert_env_to_map(&mut name_map, "COSMOS_DATABASE_NAME")?;
        let ssl_key_location = insert_env_to_map(&mut name_map, "SSL_KEY_FILE")?;
        let ssl_cert_location = insert_env_to_map(&mut name_map, "SSL_CERT_FILE")?;
        let login_secret_key = insert_env_to_map(&mut name_map, "LOGIN_SECRET_KEY")?;
        let validation_secret_key = insert_env_to_map(&mut name_map, "VALIDATION_SECRET_KEY")?;
        let rust_log = insert_env_to_map(&mut name_map, "RUST_LOG")?;
        let test_phone_number = insert_env_to_map(&mut name_map, "TEST_PHONE_NUMBER")?;
        let service_phone_number = insert_env_to_map(&mut name_map, "SERVICE_PHONE_NUMBER")?;
        let test_email = insert_env_to_map(&mut name_map, "TEST_EMAIL")?;
        let service_email = insert_env_to_map(&mut name_map, "SERVICE_FROM_EMAIL")?;
        let location = insert_env_to_map(&mut name_map, "AZURE_LOCATION")?;

        Ok(Self {
            resource_group,
            kv_name,
            test_phone_number,
            service_phone_number,
            azure_location: location,
            cosmos_token,
            cosmos_account,
            ssl_key_location,
            ssl_cert_location,
            login_secret_key,
            validation_secret_key,
            cosmos_database_name: cosmos_database,
            cosmos_collections: vec![
                "Users-Collection".to_string(),
                "Game-Collection".to_string(),
                "Profile-Collection".to_string(),
                "Users-Collection-test".to_string(),
                "Game-Collection-test".to_string(),
                "Profile-Collection-test".to_string(),
            ],
            rust_log,
            test_email,
            service_email,
            name_value_map: name_map.clone(),
        })
    }

    pub fn dump_values(&self) {
        log::info!("cosmos_token: {}", self.cosmos_token);
        log::info!("cosmos_account: {}", self.cosmos_account);
        log::info!("ssl_key_location: {}", self.ssl_key_location);
        log::info!("ssl_cert_location: {}", self.ssl_cert_location);
        log::info!("login_secret_key: {}", self.login_secret_key);
        log::info!("validation_secret_key: {}", self.validation_secret_key);
        log::info!("database_name: {}", self.cosmos_database_name);
        log::info!("rust_log: {}", self.rust_log);
        log::info!("kv_name: {}", self.kv_name);
        log::info!("test_phone_number: {}", self.test_phone_number);
        log::info!("test_email: {}", self.test_email);
        log::info!("service_mail: {}", self.service_email);
    }
}
impl Default for ConfigEnvironmentVariables {
    fn default() -> Self {
        Self {
            cosmos_token: String::default(),
            cosmos_account: "user-cosmos-account".to_owned(),
            ssl_key_location: String::default(),
            ssl_cert_location: String::default(),
            login_secret_key: String::default(),
            validation_secret_key: String::default(),
            cosmos_database_name: "Users-Database".to_owned(),
            cosmos_collections: vec![
                "Users-Collection".to_string(),
                "Game-Collection".to_string(),
                "Profile-Collection".to_string(),
                "Users-Collection-test".to_string(),
                "Game-Collection-test".to_string(),
                "Profile-Collection-test".to_string(),
            ],
            rust_log: "actix_web=trace,actix_server=trace,rust=trace".to_owned(),
            kv_name: String::default(),
            test_phone_number: String::default(),
            resource_group: "catan-rg".to_owned(),
            azure_location: "westus3".to_owned(),
            service_phone_number: String::default(),
            test_email: String::default(),
            service_email: String::default(),
            name_value_map: HashMap::<String, String>::new(),
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
