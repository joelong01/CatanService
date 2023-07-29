#![allow(dead_code)]

use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

use crate::{games_service::game_container::game_messages::{CatanMessage, ErrorData}, error_message};

lazy_static::lazy_static! {
    static ref LOBBY: Arc<RwLock<Lobby>> = Arc::new(RwLock::new(Lobby::new()));
}

pub struct Lobby {
    player_ids: Vec<String>,
    notify: broadcast::Sender<CatanMessage>,
}
impl Lobby {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(1000); // can't have more than 1000 players in the lobby!!
        Self {
            player_ids: Vec::new(),
            notify: tx
        }
    }


    pub async fn join_lobby(player_id: String) {
        let mut lobby = LOBBY.write().await;
        lobby.player_ids.push(player_id);
    }

    pub async fn leave_lobby(player_id: &str) {
        let mut lobby = LOBBY.write().await;
        if let Some(pos) = lobby.player_ids.iter().position(|x| *x == player_id) {
            lobby.player_ids.swap_remove(pos);
        }
    }

    pub async fn copy_loby() -> Vec<String> {
        let lobby = LOBBY.read().await;
        lobby.player_ids.clone()
    }

    pub async fn wait_for_invite() -> CatanMessage {
        let lobby = LOBBY.read().await;

        let mut rx = lobby.notify.subscribe();

        drop(lobby);

        let message = match rx.recv().await {
            Ok(msg) => msg,
            Err(e) => {
                error_message!(500, format!("Broadcast channel was closed: {}", e)) 
            }
        };
        message
    }
}
