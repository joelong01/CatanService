#![allow(dead_code)]

use serde::{Deserialize, Serialize};

use crate::games_service::catan_games::games::regular::regular_game::RegularGame;

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Invitation {
    pub from_id: String,
    pub to_id: String,
    pub from_name: String,
    pub message: String,
    pub picture_url: String,
    pub game_id: String
}
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GameHeaders {
    game_id: String,
    user_id: String,
    password: String,
    is_test: String,
    email: String
}
impl GameHeaders {
    pub const GAME_ID: &'static str = "x-game-id";
    pub const USER_ID: &'static str = "x-user-id";
    pub const PASSWORD: &'static str = "x-password";
    pub const IS_TEST: &'static str = "x-is-test";
    pub const EMAIL: &'static str = "x-email";
}



#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InviteAccepted {
    pub user_id: String,
    pub game_id: String
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GameCreatedData {
    pub user_id: String,
    pub game_id: String
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ErrorData {
    pub status_code: i32,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum CatanMessage {
    GameUpdate(RegularGame),
    Invite(Invitation),
    Error(ErrorData),
    InviteAccepted(InviteAccepted),
    GameCreated(GameCreatedData)
}

#[macro_export]
macro_rules! invite_message {
    ($from:expr, $to:expr, $name:expr, $msg:expr, $url:expr) => {
        CatanMessage::Invite(InviteData {
            from_id: $from.to_string(),
            to_id: $to.to_string(),
            message: $msg.to_string(),
            picture_url: $url.to_string()
            name: $name.to_string()
        })
    };
}

#[macro_export]
macro_rules! game_update_message {
    ($game:expr) => {
        CatanMessage::GameUpdate($game)
    };
}

#[macro_export]
macro_rules! error_message {
    ($status_code:expr, $msg:expr) => {
        CatanMessage::Error(ErrorData {
            status_code: $status_code,
            message: $msg.to_string(),
        })
    };
}
