#![allow(dead_code)]

use super::game_messages::{CatanMessage, ErrorData, GameHeaders};
use actix_web::{HttpRequest, HttpResponse};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

use tracing::info;

use crate::{
    error_message, game_update_message,
    games_service::{catan_games::games::regular::regular_game::RegularGame, lobby::lobby::Lobby},
    shared::models::GameError,
};

lazy_static::lazy_static! {
    static ref GAME_MAP: Arc<RwLock<HashMap<String, Arc<RwLock<GameContainer>>>>> = Arc::new(RwLock::new(HashMap::new()));
}

pub struct GameContainer {
    game_id: String,
    user_ids: Box<Vec<String>>,
    undo_stack: Vec<RegularGame>,
    redo_stack: Vec<RegularGame>,
    notify: broadcast::Sender<CatanMessage>,
}

impl GameContainer {
    pub async fn insert_container(user_id: String, game_id: String, game: &mut RegularGame) {
        {
            let game_container = GameContainer::new(user_id, game_id.to_owned());
            let mut game_map = GAME_MAP.write().await; // Acquire write lock
            game_map.insert(game_id.to_owned(), Arc::new(RwLock::new(game_container)));
        }

        GameContainer::push_game(&game_id, game).await;
    }

    fn new(user_id: String, game_id: String) -> Self {
        // Create a new broadcast channel with a capacity of 10...i don't think there are any Catan Games with > 7 players
        let (tx, _) = broadcast::channel(10);
        Self {
            game_id: game_id.clone(),
            user_ids: Box::new(vec![user_id]),
            undo_stack: vec![],
            redo_stack: vec![],
            notify: tx, // Use the Sender from the new channel
        }
    }

    pub async fn get_locked_container(
        game_id: &String,
    ) -> Result<Arc<RwLock<GameContainer>>, String> {
        let game_map = GAME_MAP.read().await; // Acquire read lock
        match game_map.get(game_id) {
            Some(container) => Ok(container.clone()),
            None => Err(format!("Game ID {} not found", game_id)),
        }
    }

    pub async fn wait_for_change(game_id: String) -> CatanMessage {
        let game_container = match Self::get_locked_container(&game_id).await {
            Ok(container) => container,
            Err(_) => return error_message!(400, "Bad Game ID".to_string()),
        };

        let mut rx = game_container.read().await.notify.subscribe();

        info!("Starting to wait for game changes");

        let message = match rx.recv().await {
            Ok(msg) => msg,
            Err(e) => {
                error_message!(500, format!("Broadcast channel was closed: {}", e))
            }
        };

        message
    }

    pub async fn add_player(game_id: &String, user_id: String) {
        let game_container = Self::get_locked_container(game_id).await.unwrap();
        let mut rw_game_container = game_container.write().await;
        rw_game_container.user_ids.push(user_id);
        let _ = rw_game_container
            .notify
            .send(game_update_message!(rw_game_container
                .undo_stack
                .last()
                .unwrap()
                .clone()));
    }

    pub async fn undo(game_id: &String) {
        let game_container = Self::get_locked_container(game_id).await.unwrap();
        let mut locked_container = game_container.write().await;
        if locked_container.undo_stack.len() < 2 {
            return;
        }
        if !locked_container.undo_stack.last().unwrap().can_undo {
            return;
        }
        let game = locked_container.undo_stack.pop().unwrap();

        locked_container.redo_stack.push(game);
        let _ = locked_container
            .notify
            .send(game_update_message!(locked_container
                .undo_stack
                .last()
                .unwrap()
                .clone()));
    }

    pub async fn current(game_id: &String) -> Result<RegularGame, GameError> {
        match Self::get_locked_container(game_id).await {
            Ok(game_container) => {
                let ro_container = game_container.read().await;
                Ok(ro_container.undo_stack.last().unwrap().clone())
            }
            Err(_) => Err(GameError::InvalidGameId),
        }
    }

    pub async fn push_game(game_id: &String, game: &RegularGame) {
        let game_container = Self::get_locked_container(game_id).await.unwrap();
        let mut rw_game_container = game_container.write().await;
        rw_game_container.undo_stack.push(game.clone());
        let _ = rw_game_container
            .notify
            .send(game_update_message!(game.clone()));
    }

    pub async fn get_players(game_id: &String) -> Box<Vec<String>> {
        let game_container = Self::get_locked_container(game_id).await.unwrap();
        let game_container = game_container.read().await;
        game_container.user_ids.clone()
    }
}
/**
 *  a GET that is a long polling get.  the call waits here until the game changes and then the service will signal
 *  and the call will complete, returning a CatanMessage.  TODO:  this should
 */
pub async fn long_poll_handler(req: HttpRequest) -> HttpResponse {
    let message = match req.headers().get(GameHeaders::GAME_ID) {
        Some(header) => {
            let game_id = header.to_str().unwrap().to_string();
            GameContainer::wait_for_change(game_id.to_owned()).await
        }
        None => 
        {
            let user_id = req.headers().get(GameHeaders::USER_ID).unwrap().to_str().unwrap();
            Lobby::wait_for_invite(user_id).await
        }
    };

    return HttpResponse::Ok()
        .content_type("application/json")
        .json(message);
}
