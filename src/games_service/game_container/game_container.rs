#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tracing::info;

use crate::{games_service::catan_games::games::regular::regular_game::RegularGame, shared::models::GameError};

lazy_static::lazy_static! {
    static ref GAME_MAP: Arc<RwLock<HashMap<String, Arc<RwLock<GameContainer>>>>> = Arc::new(RwLock::new(HashMap::new()));
}

// Get container helper

// Insert container helper

pub struct GameContainer {
    game_id: String,
    player_ids: Box<Vec<String>>,
    undo_stack: Vec<RegularGame>,
    redo_stack: Vec<RegularGame>,
    notify: broadcast::Sender<RegularGame>,
}

impl GameContainer {
    pub async fn insert_container(player_id: String, game_id: String, game: &mut RegularGame) {
        let game_container = GameContainer::new(player_id, game_id.to_owned(), game);
        let mut game_map = GAME_MAP.write().await; // Acquire write lock
        game_map.insert(game_id.to_owned(), Arc::new(RwLock::new(game_container)));
    }

    fn new(player_id: String, game_id: String, game: &RegularGame) -> Self {
       
        // Create a new broadcast channel with a capacity of 10...i don't think there are any Catan Games with > 7 players
        let (tx, _) = broadcast::channel(10);
        Self {
            game_id: game_id.clone(),
            player_ids: Box::new(vec![player_id]),
            undo_stack: vec![game.clone()],
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

    pub async fn wait_for_change(game_id: String) -> Result<RegularGame, String> {
        let game_container = match Self::get_locked_container(&game_id).await {
            Ok(container) => container,
            Err(_) => return Err("Bad Game ID".to_string()),
        };

        let mut rx = game_container.read().await.notify.subscribe();

        info!("Starting to wait for game changes");

        let game_state = match rx.recv().await {
            Ok(game) => game,
            Err(e) => {
                return Err(format!("Broadcast channel was closed: {}", e));
            }
        };

        Ok(game_state.clone())
    }

    pub async fn add_player(game_id: &String, player_id: String) {
        let game_container = Self::get_locked_container(game_id).await.unwrap();
        let mut game = game_container.write().await;
        game.player_ids.push(player_id);
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
            .send(locked_container.undo_stack.last().unwrap().clone());
    }

    pub async fn current(game_id: &String) -> Result<RegularGame, GameError> {
        match Self::get_locked_container(game_id).await {
            Ok(game_container) => {
                let ro_container = game_container.read().await;
                Ok(ro_container.undo_stack.last().unwrap().clone())
            }
            Err(_) => Err(GameError::InvalidGameId)
        }
       
    }

    pub async fn push_game(game_id: &String, game: &RegularGame) {
        let game_container = Self::get_locked_container(game_id).await.unwrap();
        let mut rw_game_container = game_container.write().await;
        rw_game_container.undo_stack.push(game.clone());
        let _ = rw_game_container.notify.send(game.clone());
    }

    pub async fn get_players(game_id: &String) -> Box<Vec<String>> {
        let game_container = Self::get_locked_container(game_id).await.unwrap();
        let game_container = game_container.read().await;
        game_container.player_ids.clone()
    }
}
