#![allow(dead_code)]

use super::game_messages::CatanMessage;
use crate::{
    games_service::{
        catan_games::games::regular::regular_game::RegularGame,
        long_poller::long_poller::LongPoller,
    },
    shared::shared_models::{UserProfile, GameError, ResponseType, ServiceResponse},

};


use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

lazy_static::lazy_static! {
    static ref GAME_MAP: Arc<RwLock<HashMap<String, Arc<RwLock<GameContainer>>>>> = Arc::new(RwLock::new(HashMap::new()));
}

pub struct GameContainer {
    game_id: String,
    undo_stack: Vec<RegularGame>,
    redo_stack: Vec<RegularGame>,
}

impl GameContainer {
    pub async fn create_and_add_container(
        game_id: &str,
        game: &RegularGame,
    ) -> Result<ServiceResponse, ServiceResponse> {
        let mut game_map = GAME_MAP.write().await; // Acquire write lock
        if game_map.contains_key(game_id) {
            return Err(ServiceResponse::new_bad_id("GameId", game_id));
        }

        let mut game_container = GameContainer::new(game_id);
        game_container.undo_stack.push(game.clone());
        game_map.insert(game_id.to_owned(), Arc::new(RwLock::new(game_container)));

        Ok(ServiceResponse::new_generic_ok("added"))
    }

    fn new(game_id: &str) -> Self {
        Self {
            game_id: game_id.to_string(),

            undo_stack: vec![],
            redo_stack: vec![],
        }
    }

    pub async fn get_locked_container(
        game_id: &str,
    ) -> Result<Arc<RwLock<GameContainer>>, ServiceResponse> {
        let game_map = GAME_MAP.read().await; // Acquire read lock
        match game_map.get(game_id) {
            Some(container) => Ok(container.clone()),
            None => Err(ServiceResponse::new_bad_id("GameId", game_id)),
        }
    }

    /**
     *  add a player to a game.  takes a write lock and writes a PlayerAdded message to the long poller.  while we have a
     *  game we could return (which has the players), at this point, I think the UI would be a "Create Game" UI where invites
     *  have been sent out and the UI reflects updates based on Accept/Reject
     */
    pub async fn add_player(
        game_id: &str,
        client_user: &UserProfile,
    ) -> Result<ServiceResponse, ServiceResponse> {


        let game_container = Self::get_locked_container(&game_id).await?;
        let mut game_container = game_container.write().await; // drop locked container

        let game = game_container.undo_stack.last().clone().unwrap(); // you cannot have an empty undo stack *and a valid game_id
        let clone = game.add_user(client_user)?;
        game_container.undo_stack.push(clone.clone());
        Ok(ServiceResponse::new_generic_ok("added"))
    }
    /**
     *  send the message to all players in game_id
     */
    pub async fn broadcast_message(
        game_id: &str,
        message: &CatanMessage,
    ) -> Result<ServiceResponse, ServiceResponse> {
        let ids = GameContainer::get_game_players(game_id).await?;
        LongPoller::send_message(ids, message).await
    }

    pub async fn get_game_players(game_id: &str) -> Result<Vec<String>, ServiceResponse> {
        let mut players = Vec::new();
        let (game, _) = GameContainer::current_game(game_id).await?;
        for p in game.players.values() {
            players.push(p.profile.user_id.clone().unwrap());
        }

        Ok(players)
    }

    pub async fn undo(game_id: &String) -> Result<ServiceResponse, ServiceResponse> {
        let game_container = Self::get_locked_container(game_id).await?;
        let mut game_container = game_container.write().await;
        let len = game_container.undo_stack.len();
        if len < 2 {
            return Err(ServiceResponse::new(
                "",
                reqwest::StatusCode::BAD_REQUEST,
                ResponseType::NoData,
                GameError::ActionError(format!("cannot undo first game. undo_stack len {} ", len)),
            ));
        }
        if !game_container.undo_stack.last().unwrap().can_undo {
            return Err(ServiceResponse::new(
                "",
                reqwest::StatusCode::BAD_REQUEST,
                ResponseType::NoData,
                GameError::ActionError("item in undo stack cannot be undone".to_string()),
            ));
        }
        let game = game_container.undo_stack.pop().unwrap();

        game_container.redo_stack.push(game.clone());
        let _ = Self::broadcast_message(game_id, &CatanMessage::GameUpdate(game)).await;
        Ok(ServiceResponse::new_generic_ok(""))
    }

    pub async fn current_game(game_id: &str) -> Result<(RegularGame, bool), ServiceResponse> {
        match Self::get_locked_container(game_id).await {
            Ok(game_container) => {
                let ro_container = game_container.read().await;
                Ok((
                    ro_container.undo_stack.last().unwrap().clone(),
                    ro_container.redo_stack.len() > 0,
                ))
            }
            Err(_) => Err(ServiceResponse::new(
                "",
                reqwest::StatusCode::NOT_FOUND,
                ResponseType::NoData,
                GameError::BadId(format!("{} not found", game_id)),
            )),
        }
    }

    pub async fn push_game(game_id: &str, game: &RegularGame) -> Result<(), ServiceResponse> {
        let game_container = Self::get_locked_container(game_id).await?;
        let mut rw_game_container = game_container.write().await;
        let game_clone = game.clone();
        rw_game_container.undo_stack.push(game_clone);
        rw_game_container.redo_stack.clear();
        drop(rw_game_container);
        let _ = Self::broadcast_message(game_id, &CatanMessage::GameUpdate(game.clone())).await;
        Ok(())
    }
}
