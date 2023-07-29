#![allow(dead_code)]

use serde::{Deserialize, Serialize};

use crate::games_service::catan_games::games::regular::regular_game::RegularGame;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InviteData {
    pub from: String,
    pub to: String,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ErrorData {
    pub status_code: i32,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum CatanMessage {
    GameUpdate(RegularGame),
    Invite(InviteData),
    Error(ErrorData),
}

#[macro_export]
macro_rules! invite_message {
    ($from:expr, $to:expr) => {
        CatanMessage::Invite(InviteData {
            from: $from.to_string(),
            to: $to.to_string(),
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
