#![allow(dead_code)]
#![allow(unused_imports)]
use super::game_messages::CatanMessage;
use crate::{
    cosmos_db::{
        cosmosdb::CosmosDb,
        database_abstractions::{DatabaseWrapper, GameDbTrait},
        mocked_db::TestDb,
    },
    full_info,
    games_service::{
        catan_games::games::regular::regular_game::RegularGame,
        long_poller::long_poller::LongPoller,
    },
    middleware::{
        request_context_mw::{RequestContext, TestContext},
        service_config::SERVICE_CONFIG,
    },
    shared::{
        service_models::PersistGame,
        shared_models::{ServiceError, UserProfile, UserType},
    },
};

use std::sync::Arc;
use std::{collections::HashMap, io::Write};
use tokio::sync::RwLock;

use flate2::{write::ZlibEncoder, Compression};
use serde::{Deserialize, Serialize}; // Assume you're using Serde for data serialization
use tokio::sync::mpsc;

//
//  this lets you find a GameContainer given a game_id
lazy_static::lazy_static! {
    static ref GAME_MAP: Arc<RwLock<HashMap<String, Arc<RwLock<GameContainer>>>>> = Arc::new(RwLock::new(HashMap::new()));
}

/// A data structure managing undo and redo operations for game states.
#[derive(Debug, Serialize, Deserialize)]
pub struct GameStacks {
    undo_stack: Vec<RegularGame>,
    redo_stack: Vec<RegularGame>,
    #[serde(skip_serializing, with = "stack_sender")]
    db_update_sender: mpsc::Sender<PersistGame>, // This will send compressed data
}

mod stack_sender {
    use tokio::sync::mpsc;

    use crate::shared::service_models::PersistGame;

    pub fn serialize<S>(
        _sender: &mpsc::Sender<PersistGame>,
        _serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        panic!("Should not serialize this field");
    }

    pub fn deserialize<'de, D>(_deserializer: D) -> Result<mpsc::Sender<PersistGame>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let (sender, _receiver) = mpsc::channel(32);
        Ok(sender)
    }
}

impl Default for GameStacks {
    fn default() -> Self {
        let (sender, _receiver) = mpsc::channel(32); // Some arbitrary buffer size

        GameStacks {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            db_update_sender: sender,
        }
    }
}

impl GameStacks {
    /// Creates a new `Stacks` instance.
    pub fn new(game_id: &str, request_context: &RequestContext) -> Self {
        let (tx, rx) = mpsc::channel(32);
        let test_context = request_context.test_context.clone();
        let game_id = game_id.to_string();
        // Spawn the database update task
        tokio::spawn(async move {
            GameStacks::db_update_task(game_id, rx, test_context).await;
        });
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            db_update_sender: tx,
        }
    }
    async fn db_update_task(
        game_id: String,
        mut rx: mpsc::Receiver<PersistGame>,
        test_context: Option<TestContext>,
    ) {
        let game_id = game_id.to_string();
        let database = DatabaseWrapper::new(&test_context, &SERVICE_CONFIG);

        while let Some(persisted_game) = rx.recv().await {
            let result = database
                .as_game_db()
                .update_game_data(&game_id, &persisted_game)
                .await;
            if result.is_err() {
                log::error!("failed to save to database: {:#?}", result);
            }
        }
    }

    async fn update_db(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let serialized_data = serde_json::to_vec(self)?;

        let decompressed_size = serialized_data.len() >> 10; // Get the size of serialized data in KB

        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&serialized_data)?;
        let compressed_data = encoder.finish()?;

        let compressed_size = compressed_data.len() >> 10; // Get the size of compressed data in KB

        // Log the sizes
        full_info!(
            "Decompressed size: {}KB Compressed size: {}KB",
            decompressed_size,
            compressed_size
        );
        let current_game = self
            .current()
            .await
            .expect("current must exist if we are trying to save current");
        let persisted_game = PersistGame::new(
            &current_game.game_id,
            current_game.game_index,
            self.undo_stack.len(),
            self.redo_stack.len(),
            compressed_size,
            decompressed_size,
            &compressed_data,
        );
        self.db_update_sender.send(persisted_game).await?;
        Ok(())
    }

    fn deserialize_sender<'de, D>(deserializer: D) -> Result<mpsc::Sender<Vec<u8>>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let _ = deserializer;
        let (sender, _receiver) = mpsc::channel(32);
        Ok(sender)
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
        let _ = self.update_db().await;
    }

    /// Moves the current game from the `undo_stack` to the `redo_stack` and returns the updated current game.
    ///
    /// Returns an error if there are fewer than two games in the `undo_stack` or if the current game cannot be undone.
    pub async fn undo(&mut self) -> Result<RegularGame, ServiceError> {
        let can_undo = self.can_undo().await?;
        if !can_undo {
            return Err(ServiceError::new_container_error(
                "current state does not allow an undo",
            ));
        }
        let game = self.undo_stack.pop().unwrap();
        self.redo_stack.push(game.clone());
        let _ = self.update_db().await;
        self.current().await
    }

    /// Pops a game from the `redo_stack`, pushes it onto the `undo_stack`, and returns the game.
    ///
    /// Returns an error if the `redo_stack` is empty.
    pub async fn redo(&mut self) -> Result<RegularGame, ServiceError> {
        let len = self.redo_stack.len();
        if len < 2 {
            // 2 because we do not undo a created game -- create a new game instead.
            return Err(ServiceError::new_container_error(
                "nothing to redo in this container",
            ));
        }
        let game = self.redo_stack.pop().unwrap();
        self.undo_stack.push(game.clone());
        let current = self.current().await?;
        assert_eq!(game.game_index, current.game_index);
        let _ = self.update_db().await;
        Ok(current)
    }

    /// Returns the last item on the `undo_stack` as the current game.
    ///
    /// Returns an error if the `undo_stack` is empty.
    pub async fn current(&self) -> Result<RegularGame, ServiceError> {
        match self.undo_stack.last() {
            Some(g) => Ok(g.clone()),
            None => Err(ServiceError::new_container_error(
                "no current game available in this container!",
            )),
        }
    }

    pub async fn can_undo(&self) -> Result<bool, ServiceError> {
        let len = self.undo_stack.len();
        if len < 2 {
            return Ok(false);
        }
        let current = self.current().await?;

        Ok(current.can_undo)
    }

    pub async fn can_redo(&self) -> Result<bool, ServiceError> {
        Ok(self.redo_stack.len() > 0)
    }
}

pub struct GameContainer {
    game_id: String,
    stacks: GameStacks,
}

impl GameContainer {
    pub async fn create_and_add_container(
        game_id: &str,
        game: &RegularGame,
        request_context: &RequestContext,
    ) -> Result<(), ServiceError> {
        let mut game_map = GAME_MAP.write().await; // Acquire write lock
        if game_map.contains_key(game_id) {
            return Err(ServiceError::new_not_found("GameId", game_id));
        }

        let mut game_container = GameContainer::new(game_id, request_context);

        game_container.stacks.push_game(&game).await;
        game_map.insert(game_id.to_owned(), Arc::new(RwLock::new(game_container)));

        Ok(())
    }

    fn new(game_id: &str, request_context: &RequestContext) -> Self {
        Self {
            game_id: game_id.to_string(),
            stacks: GameStacks::new(game_id, request_context),
        }
    }

    pub async fn get_locked_container(
        game_id: &str,
    ) -> Result<Arc<RwLock<GameContainer>>, ServiceError> {
        let game_map = GAME_MAP.read().await; // Acquire read lock
        match game_map.get(game_id) {
            Some(container) => Ok(container.clone()),
            None => Err(ServiceError::new_not_found("GameId", game_id)),
        }
    }

    /**
     *  add a player to a game.  takes a write lock and writes a PlayerAdded message to the long poller.  while we have a
     *  game we could return (which has the players), at this point, I think the UI would be a "Create Game" UI where invites
     *  have been sent out and the UI reflects updates based on Accept/Reject
     */
    pub async fn add_player(game_id: &str, client_user: &UserProfile) -> Result<(), ServiceError> {
        let game_container = Self::get_locked_container(&game_id).await?;
        let mut game_container = game_container.write().await; // drop read lock

        let mut game = game_container.stacks.current().await?;
        game.add_user(client_user)?;
        game_container.stacks.push_game(&game).await;
        drop(game_container);
        let keys_vec: Vec<String> = game.players.keys().cloned().collect();
        let _ = Self::broadcast_message(game_id, &CatanMessage::PlayerAdded(keys_vec)).await;
        Ok(())
    }
    /**
     *  send the message to all connected players in game_id
     */
    pub async fn broadcast_message(
        game_id: &str,
        message: &CatanMessage,
    ) -> Result<(), ServiceError> {
        let ids = GameContainer::get_connected_players(game_id).await?;

        LongPoller::send_message(ids, message).await?;
        Ok(())
    }
    pub async fn get_connected_players(game_id: &str) -> Result<Vec<String>, ServiceError> {
        let (game, _) = GameContainer::current_game(game_id).await?;
        let connected_players: Vec<String> = game
            .players
            .values()
            .filter(|&player| matches!(player.profile.user_type, UserType::Connected))
            .filter_map(|player| player.profile.user_id.clone())
            .collect();

        Ok(connected_players)
    }
    pub async fn get_game_players(game_id: &str) -> Result<Vec<String>, ServiceError> {
        let mut players = Vec::new();
        let (game, _) = GameContainer::current_game(game_id).await?;
        for p in game.players.values() {
            players.push(p.profile.user_id.clone().unwrap());
        }

        Ok(players)
    }

    pub async fn undo(game_id: &String) -> Result<(), ServiceError> {
        let game_container = Self::get_locked_container(game_id).await?;
        let mut game_container = game_container.write().await;
        let game = game_container.stacks.undo().await?;
        drop(game_container);
        let _ = Self::broadcast_message(game_id, &CatanMessage::GameUpdate(game)).await;
        Ok(())
    }

    pub async fn current_game(game_id: &str) -> Result<(RegularGame, bool), ServiceError> {
        let game_container = Self::get_locked_container(game_id).await?;
        let game_container = game_container.read().await;
        Ok((
            game_container.stacks.current().await?,
            game_container.stacks.can_undo().await?,
        ))
    }

    pub async fn push_game(game_id: &str, game: &RegularGame) -> Result<(), ServiceError> {
        let game_container = Self::get_locked_container(game_id).await?;
        let mut game_container = game_container.write().await;
        game_container.stacks.push_game(&game).await;
        drop(game_container);
        let _ = Self::broadcast_message(game_id, &CatanMessage::GameUpdate(game.clone())).await;
        Ok(())
    }

    pub async fn load_game(
        game_id: &str,
        _request_context: &RequestContext,
    ) -> Result<(), ServiceError> {
        let response = GameContainer::current_game(&game_id.to_owned()).await;

        match response {
            Ok(_) => {
                return Ok(());
            }
            Err(_) => {
                // let game_stacks = request_context.user_database.
            }
        }

        todo!();
    }
}
