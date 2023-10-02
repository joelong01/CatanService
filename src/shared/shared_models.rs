#![allow(dead_code)]
use super::service_models::PersistUser;
/**
 * this is the module where I define the structures needed for the data in Cosmos
 */
use actix_web::HttpResponse;
use anyhow::Result;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use std::{fmt, fmt::Display, fmt::Formatter};
use strum_macros::Display;
use crate::shared::shared_models::ResponseType::AzError;
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
    HttpError,
    AzError(String),
    JsonError(String),
    AzureCoreError(String),
    ContainerError(String),
}

// we need a From<> for each error type we add to use the error propagation ?
impl From<reqwest::Error> for GameError {
    fn from(err: reqwest::Error) -> Self {
        GameError::ReqwestError(format!("{:#?}", err))
    }
}

impl From<serde_json::Error> for GameError {
    fn from(err: serde_json::Error) -> Self {
        GameError::JsonError(err.to_string())
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
            GameError::HttpError => write!(f, "HttpError"),
            GameError::AzError(e) => write!(f, "AzError: {:#?}", e),
            GameError::JsonError(e) => write!(f, "Serde Error: {:#?}", e),
            GameError::AzureCoreError(e) => write!(f, "Azure Core error: {:#?}", e),
            GameError::ContainerError(e) => write!(f, "GameContainer Error: {:#?}", e),
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

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub struct PersonalInformation {
    pub phone_number: String,
    pub email: String,
    pub first_name: String,
    pub last_name: String,
}

impl PersonalInformation {
    pub fn is_equal_by_val(&self, other: &PersonalInformation) -> bool {
        if self.phone_number == other.phone_number
            && self.email == other.email
            && self.first_name == other.first_name
            && self.last_name == other.last_name
        {
            return true;
        }

        false
    }
}

///
/// UserProfile is just information about the client.  this can be as much or little information as the app needs
/// to run
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub struct UserProfile {
    pub user_id: Option<String>,
    pub user_type: UserType,
    pub pii: Option<PersonalInformation>,
    pub display_name: String,
    pub picture_url: String,
    pub foreground_color: String,
    pub background_color: String,
    pub text_color: String,
    pub games_played: Option<u16>,
    pub games_won: Option<u16>,
    pub validated_email: bool, // has the mail been validated?
    pub validated_phone: bool, // has the phone number been validated?
}
impl Default for UserProfile {
    fn default() -> Self {
        UserProfile {
            user_type: UserType::Connected,
            user_id: None,
            pii: None,
            display_name: String::default(),
            picture_url: String::default(),
            foreground_color: String::default(),
            background_color: String::default(),
            text_color: String::default(),
            games_played: None,
            games_won: None,
            validated_email: false,
            validated_phone: false,
        }
    }
}

impl UserProfile {
    pub fn is_equal_by_val(&self, other: &UserProfile) -> bool {
        match &self.pii {
            Some(pii) => match &other.pii {
                Some(other_pii) => {
                    if !pii.is_equal_by_val(other_pii) {
                        return false;
                    }
                }
                None => return false,
            },
            None => {
                if other.pii.is_some() {
                    return false;
                }
            }
        }

        self.display_name == other.display_name
            && self.picture_url == other.picture_url
            && self.foreground_color == other.foreground_color
            && self.background_color == other.background_color
            && self.text_color == other.text_color
            && self.games_played.unwrap_or(0) == other.games_played.unwrap_or(0)
            && self.games_won.unwrap_or(0) == other.games_won.unwrap_or(0)
            && self.validated_email == other.validated_email
            && self.validated_phone == other.validated_phone
    }

    pub fn get_email_or_panic(&self) -> String {
        match &self.pii {
            Some(pii) => pii.email.clone(),
            None => panic!("Asked for email and it doesn't exist"),
        }
    }
    pub fn update_from(&mut self, other: &UserProfile) {
        // For non-optional fields, update directly:
        self.display_name = other.display_name.clone();
        self.picture_url = other.picture_url.clone();
        self.foreground_color = other.foreground_color.clone();
        self.background_color = other.background_color.clone();
        self.text_color = other.text_color.clone();
        self.validated_email = other.validated_email;
        self.validated_phone = other.validated_phone;

        // Update optional fields if the other has a value:
        if let Some(ref other_id) = other.user_id {
            self.user_id = Some(other_id.clone());
        }

        if let Some(ref other_games_played) = other.games_played {
            self.games_played = Some(*other_games_played);
        }

        if let Some(ref other_games_won) = other.games_won {
            self.games_won = Some(*other_games_won);
        }

        if let Some(ref other_pii) = other.pii {
            if self.pii.is_none() {
                self.pii = Some(other_pii.clone());
            } else {
                // Update the fields of pii if self has it
                let self_pii = self.pii.as_mut().unwrap();
                self_pii.email = other_pii.email.clone();
                self_pii.phone_number = other_pii.phone_number.clone();
                self_pii.first_name = other_pii.first_name.clone();
                self_pii.last_name = other_pii.last_name.clone();
            }
        } else {
            // If the other's pii is None, set self's pii to None as well
            self.pii = None;
        }
    }
    pub fn from_persist_user(persist_user: &PersistUser) -> Self {
        persist_user.user_profile.clone()
        // Self {
        //     user_id: persist_user.user_profile.user_id.clone(),
        //     user_type: persist_user.user_profile.user_type.clone(),
        //     pii: persist_user.user_profile.pii.clone(),
        //     display_name: persist_user.user_profile.display_name.clone(),
        //     picture_url: persist_user.user_profile.picture_url.clone(),
        //     foreground_color: persist_user.user_profile.foreground_color.clone(),
        //     background_color: persist_user.user_profile.background_color.clone(),
        //     text_color: persist_user.user_profile.text_color.clone(),
        //     games_played: persist_user.user_profile.games_played.clone(),
        //     games_won: persist_user.user_profile.games_won.clone(),
        // }
    }

    pub fn new_test_user(id: Option<String>) -> Self {
        let id = match id {
            Some(id) => id,
            None => PersistUser::new_id(),
        };
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
            user_id: Some(id),
            pii: Some(PersonalInformation {
                email: format!("{}@test.com", random_string()),
                phone_number: random_string(),
                first_name: random_name.clone(),
                last_name: random_name.clone(),
            }),

            display_name: random_name,
            picture_url: String::default(),
            foreground_color: String::default(),
            background_color: String::default(),
            text_color: String::default(),
            games_played: None,
            games_won: None,
            validated_email: false,
            validated_phone: false,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq, Display)]
pub enum ResponseType {
    ErrorInfo(String),
    Todo(String),
    SendMessageError(Vec<(String, GameError)>),
    AzError(String),
    SerdeError(String),
    NoData,
}

/**
 *  We want every response to be in JSON format so that it is easier to script calling the service.
 */
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub struct ServiceError {
    pub message: String,
    #[serde(serialize_with = "serialize_status_code")]
    #[serde(deserialize_with = "deserialize_status_code")]
    pub status: reqwest::StatusCode,
    pub response_type: ResponseType,
    pub game_error: GameError,
}
impl Display for ServiceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        // Convert the struct to a pretty-printed JSON string.
        let json = serde_json::to_string_pretty(self).map_err(|_| fmt::Error)?;
        write!(f, "{}", json)
    }
}

impl From<serde_json::Error> for ServiceError {
    fn from(err: serde_json::Error) -> Self {
        ServiceError::new(
            "Unable to deserialize response",
            StatusCode::INTERNAL_SERVER_ERROR,
            ResponseType::NoData,
            GameError::JsonError(err.to_string()),
        )
    }
}

impl ServiceError {
    pub fn new(
        message: &str,
        status: StatusCode,
        response_type: ResponseType,
        error: GameError,
    ) -> Self {
        ServiceError {
            message: message.into(),
            status,
            response_type,
            game_error: error,
        }
    }

    pub fn new_internal_server_fault(msg: &str) -> Self {
        ServiceError {
            message: msg.to_owned(),
            status: StatusCode::INTERNAL_SERVER_ERROR,
            response_type: ResponseType::NoData,
            game_error: GameError::HttpError,
        }
    }

    pub fn new_not_found(msg: &str, id: &str) -> Self {
        ServiceError {
            message: msg.to_owned(),
            status: StatusCode::NOT_FOUND,
            response_type: ResponseType::NoData,
            game_error: GameError::BadId(id.to_owned()),
        }
    }
    pub fn new_bad_request(msg: &str) -> Self {
        ServiceError {
            message: msg.to_owned(),
            status: StatusCode::BAD_REQUEST,
            response_type: ResponseType::NoData,
            game_error: GameError::HttpError,
        }
    }
    pub fn new_unauthorized(msg: &str) -> Self {
        ServiceError {
            message: msg.to_owned(),
            status: StatusCode::UNAUTHORIZED,
            response_type: ResponseType::NoData,
            game_error: GameError::HttpError,
        }
    }

    pub fn new_az_error(msg: &str) -> Self {
        ServiceError {
        message: String::default(),
            status: StatusCode::BAD_REQUEST,
            response_type: AzError(msg.to_string()),
            game_error: GameError::AzError(msg.to_string()),
        }
    }

    pub fn new_conflict_error(msg: &str) -> Self {
        ServiceError {
            message: msg.to_string(),
            status: StatusCode::CONFLICT,
            response_type: ResponseType::NoData,
            game_error: GameError::HttpError,
        }
    }

    pub fn new_container_error(msg: &str) -> Self {
        ServiceError {
            message: msg.to_string(),
            status: StatusCode::BAD_REQUEST,
            response_type: ResponseType::NoData,
            game_error: GameError::ContainerError(String::default()),
        }
    }

    pub fn new_database_error(msg: &str, error: &str) -> Self {
        ServiceError {
            message: msg.to_string(),
            status: StatusCode::INTERNAL_SERVER_ERROR,
            response_type: ResponseType::ErrorInfo(error.to_string()),
            game_error: GameError::HttpError,
        }
    }

    pub fn new_json_error(msg: &str, error: &serde_json::Error) -> Self {
        ServiceError {
            message: msg.to_string(),
            status: StatusCode::INTERNAL_SERVER_ERROR,
            response_type: ResponseType::ErrorInfo(error.to_string()),
            game_error: GameError::JsonError(format!("{:#?}", error)),
        }
    }
    pub fn new_azure_core_error(msg: &str, error: &azure_core::Error) -> Self {
        ServiceError {
            message: msg.to_string(),
            status: StatusCode::INTERNAL_SERVER_ERROR,
            response_type: ResponseType::AzError(error.to_string()),
            game_error: GameError::AzureCoreError(format!("{:#?}", error)),
        }
    }

    pub fn to_http_response(&self) -> HttpResponse {
        let serialized = serde_json::to_string(&self).expect("Failed to serialize Response Type");
        let response = HttpResponse::build(self.status).body(serialized);
        response
    }

    pub fn to_error_info(json: &str) -> Option<(ServiceError, String)> {
        let service_error: ServiceError = match serde_json::from_str(json) {
            Ok(sr) => sr,
            Err(_) => return None,
        };
        match service_error.response_type.clone() {
            ResponseType::ErrorInfo(error_info) => Some((service_error, error_info)),
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
