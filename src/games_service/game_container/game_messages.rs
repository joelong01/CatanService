#![allow(dead_code)]

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::games_service::catan_games::games::regular::regular_game::RegularGame;

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Invitation {
    pub from_id: String,
    pub to_id: String,
    pub from_name: String,
    pub to_name: String,
    pub message: String,
    pub from_picture: String,
    pub game_id: String,
}
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GameHeader {
    game_id: String,
    user_id: String,
    password: String,
    is_test: String,
    email: String,
}
impl GameHeader {
    pub const GAME_ID: &'static str = "x-game-id";
    pub const USER_ID: &'static str = "x-user-id";
    pub const PASSWORD: &'static str = "x-password";
    pub const IS_TEST: &'static str = "x-is-test";
    pub const EMAIL: &'static str = "x-email";
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InvitationResponseData {
    pub from_id: String,
    pub to_id: String,
    pub from_name: String,
    pub to_name: String,
    pub game_id: String,
    pub accepted: bool,
}
impl InvitationResponseData {
    pub fn new(
        from: &str,
        to: &str,
        from_name: &str,
        to_name: &str,
        accepted: bool,
        game_id: &str,
    ) -> Self {
        Self {
            accepted,
            to_id: to.into(),
            from_id: from.into(),
            game_id: game_id.into(),
            from_name: from_name.into(),
            to_name: to_name.into(),
        }
    }
    pub fn from_invitation(accepted: bool, invite: &Invitation) -> Self {
        Self {
            from_id: invite.to_id.clone(),
            to_id: invite.from_id.clone(),
            game_id: invite.game_id.clone(),
            accepted: accepted,
            from_name: invite.to_name.clone(),
            to_name: invite.from_name.clone(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GameCreatedData {
    pub user_id: String,
    pub game_id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ErrorData {
    pub status_code: i32,
    pub message: String,
}

#[derive(Serialize, Deserialize, Clone, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum CatanMessage {
    GameUpdate(RegularGame),
    Invite(Invitation),
    InvitationResponse(InvitationResponseData),
    GameCreated(GameCreatedData),
    PlayerAdded(Vec<String>),
    Started(String),
    Ended(String),
    Error(ErrorData),
}
impl fmt::Debug for CatanMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CatanMessage::GameUpdate(game) => write!(
                f,
                "GameUpdate: [id={}] [player_count={}] [index={}]",
                game.id,
                game.players.len(),
                game.game_index
            ),
            CatanMessage::Invite(invitation) => write!(f, "Invite: {:?}", invitation),
            CatanMessage::InvitationResponse(response) => {
                write!(f, "InvitationResponse: {:?}", response)
            }
            CatanMessage::GameCreated(data) => write!(f, "GameCreated: {:?}", data),
            CatanMessage::PlayerAdded(players) => write!(f, "PlayerAdded: {:?}", players),
            CatanMessage::Started(started) => write!(f, "Started: {}", started),
            CatanMessage::Ended(ended) => write!(f, "Ended: {}", ended),
            CatanMessage::Error(error) => write!(f, "Error: {:?}", error),
        }
    }
}
#[derive(Serialize, Deserialize, Clone, Eq, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub enum GameStatus {
    PlayingGame,
    Available,
    Hidden
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

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LobbyUser {
    pub user_id: String,
    pub user_name: String,
}
