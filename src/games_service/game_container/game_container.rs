#![allow(dead_code)]

use super::game_messages::CatanMessage;
use crate::{
    games_service::{
        catan_games::games::regular::regular_game::RegularGame,
        long_poller::long_poller::LongPoller,
    },
    shared::shared_models::{ServiceResponse, UserProfile, UserType},
};

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

//
//  this lets you find a GameContainer given a game_id
lazy_static::lazy_static! {
    static ref GAME_MAP: Arc<RwLock<HashMap<String, Arc<RwLock<GameContainer>>>>> = Arc::new(RwLock::new(HashMap::new()));
}

/// A data structure managing undo and redo operations for game states.
pub struct Stacks {
    undo_stack: Vec<RegularGame>,
    redo_stack: Vec<RegularGame>,
}

impl Stacks {
    /// Creates a new `Stacks` instance.
    pub fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    /// Pushes a game onto the `undo_stack`, making it the current game.
    ///
    /// This operation clears the `redo_stack`.
    ///
    /// # Arguments
    ///
    /// * `game` - The game state to be pushed onto the `undo_stack`.
    pub async fn push_game(&mut self, game: &RegularGame) {
        let mut clone = game.clone();
        clone.game_index = clone.game_index + 1;
        self.undo_stack.push(clone);
        self.redo_stack.clear();
    }

    /// Moves the current game from the `undo_stack` to the `redo_stack` and returns the updated current game.
    ///
    /// Returns an error if there are fewer than two games in the `undo_stack` or if the current game cannot be undone.
    pub async fn undo(&mut self) -> Result<RegularGame, ServiceResponse> {
        let can_undo = self.can_undo().await?;
        if !can_undo {
            return Err(ServiceResponse::new_container_error(
                "current state does not allow an undo",
            ));
        }
        let game = self.undo_stack.pop().unwrap();
        self.redo_stack.push(game.clone());
        self.current().await
    }

    /// Pops a game from the `redo_stack`, pushes it onto the `undo_stack`, and returns the game.
    ///
    /// Returns an error if the `redo_stack` is empty.
    pub async fn redo(&mut self) -> Result<RegularGame, ServiceResponse> {
        let len = self.redo_stack.len();
        if len < 2 {
            // 2 because we do not undo a created game -- create a new game instead.
            return Err(ServiceResponse::new_container_error(
                "nothing to redo in this container",
            ));
        }
        let game = self.redo_stack.pop().unwrap();
        self.undo_stack.push(game.clone());
        let current = self.current().await?;
        assert_eq!(game.game_index, current.game_index);
        Ok(current)
    }

    /// Returns the last item on the `undo_stack` as the current game.
    ///
    /// Returns an error if the `undo_stack` is empty.
    pub async fn current(&self) -> Result<RegularGame, ServiceResponse> {
        match self.undo_stack.last() {
            Some(g) => Ok(g.clone()),
            None => Err(ServiceResponse::new_container_error(
                "no current game available in this container!",
            )),
        }
    }

    pub async fn can_undo(&self) -> Result<bool, ServiceResponse> {
        let len = self.undo_stack.len();
        if len < 2 {
            return Ok(false);
        }
        let current = self.current().await?;

        Ok(current.can_undo)
    }

    pub async fn can_redo(&self) -> Result<bool, ServiceResponse> {
        Ok(self.redo_stack.len() > 0)
    }
}

pub struct GameContainer {
    game_id: String,
    stacks: Stacks,
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

        game_container.stacks.push_game(&game).await;
        game_map.insert(game_id.to_owned(), Arc::new(RwLock::new(game_container)));

        Ok(ServiceResponse::new_generic_ok("added"))
    }

    fn new(game_id: &str) -> Self {
        Self {
            game_id: game_id.to_string(),
            stacks: Stacks::new(),
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
        let mut game_container = game_container.write().await; // drop read lock

        let mut game = game_container.stacks.current().await?;
        game.add_user(client_user)?;
        game_container.stacks.push_game(&game).await;
        drop(game_container);
        let keys_vec: Vec<String> = game.players.keys().cloned().collect();
        let _ = Self::broadcast_message(game_id, &CatanMessage::PlayerAdded(keys_vec)).await;
        Ok(ServiceResponse::new_generic_ok("added"))
    }
    /**
     *  send the message to all connected players in game_id
     */
    pub async fn broadcast_message(
        game_id: &str,
        message: &CatanMessage,
    ) -> Result<ServiceResponse, ServiceResponse> {
        let ids = GameContainer::get_connected_players(game_id).await?;

        LongPoller::send_message(ids, message).await
    }
    pub async fn get_connected_players(game_id: &str) -> Result<Vec<String>, ServiceResponse> {
        let (game, _) = GameContainer::current_game(game_id).await?;
        let connected_players: Vec<String> = game
            .players
            .values()
            .filter(|&player| matches!(player.profile.user_type, UserType::Connected))
            .filter_map(|player| player.profile.user_id.clone())
            .collect();

        Ok(connected_players)
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
        let game = game_container.stacks.undo().await?;
        drop(game_container);
        let _ = Self::broadcast_message(game_id, &CatanMessage::GameUpdate(game)).await;
        Ok(ServiceResponse::new_generic_ok(""))
    }

    pub async fn current_game(game_id: &str) -> Result<(RegularGame, bool), ServiceResponse> {
        let game_container = Self::get_locked_container(game_id).await?;
        let game_container = game_container.read().await;
        Ok((
            game_container.stacks.current().await?,
            game_container.stacks.can_undo().await?,
        ))
    }

    pub async fn push_game(game_id: &str, game: &RegularGame) -> Result<(), ServiceResponse> {
        let game_container = Self::get_locked_container(game_id).await?;
        let mut game_container = game_container.write().await;
        game_container.stacks.push_game(&game).await;
        drop(game_container);
        let _ = Self::broadcast_message(game_id, &CatanMessage::GameUpdate(game.clone())).await;
        Ok(())
    }
}
