#![allow(dead_code)]

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::games_service::catan_games::games::regular::regular_game::RegularGame;

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Invitation {
    pub originator_id: String,
    pub recipient_id: String,
    pub originator_name: String,
    pub message: String,
    pub picture_url: String,
    pub game_id: String,
}
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GameHeaders {
    game_id: String,
    user_id: String,
    password: String,
    is_test: String,
    email: String,
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
pub struct InvitationResponseData {
    pub originator_id: String,
    pub recipient_id: String,
    pub game_id: String,
    pub player_ids: Vec<String>,
    pub accepted: bool
}
impl InvitationResponseData {
    pub fn new(originator: &str, recipient: &str,  accepted: bool, game_id: &str, players: Vec<String>)->Self {
        Self {
            accepted,
            recipient_id: recipient.into(),
            originator_id: originator.into(),
            game_id: game_id.into(), // Fixed from `game_id.info()`, which seemed incorrect
            player_ids: players
        }
    }
    pub fn from_invitation(accepted: bool, invite: &Invitation) -> Self {
        let players = vec![invite.recipient_id.clone(), invite.originator_id.clone()];
        Self {
            originator_id: invite.originator_id.clone(),
            recipient_id: invite.recipient_id.clone(),
            game_id: invite.game_id.clone(),
            player_ids:players,
            accepted: accepted
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
            CatanMessage::GameUpdate(game) => write!(f, "GameUpdate: {:?}", game),
            CatanMessage::Invite(invitation) => write!(f, "Invite: {:?}", invitation),
            CatanMessage::InvitationResponse(response) => write!(f, "InvitationResponse: {:?}", response),
            CatanMessage::GameCreated(data) => write!(f, "GameCreated: {:?}", data),
            CatanMessage::PlayerAdded(players) => write!(f, "PlayerAdded: {:?}", players),
            CatanMessage::Started(started) => write!(f, "Started: {}", started),
            CatanMessage::Ended(ended) => write!(f, "Ended: {}", ended),
            CatanMessage::Error(error) => write!(f, "Error: {:?}", error),
        }
    }
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
