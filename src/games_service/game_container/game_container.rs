#![allow(dead_code)]

use super::game_messages::{CatanMessage, GameHeader};
use crate::{
    full_info,
    games_service::{catan_games::games::regular::regular_game::RegularGame, lobby::lobby::Lobby},
    shared::models::LongPollUser,
    shared::models::{ClientUser, GameError},
    trace_function,
};
use actix_web::{HttpRequest, HttpResponse};
use log::error;
use scopeguard::defer;
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
    visitors: HashMap<String, LongPollUser>,
}

impl GameContainer {
    pub async fn create_and_add_container(
        game_id: &str,
        game: &RegularGame,
    ) -> Result<(), GameError> {
        let mut game_map = GAME_MAP.write().await; // Acquire write lock
        if game_map.contains_key(game_id) {
            return Err(GameError::BadId(format!("{} already exists", game_id)));
        }

        let mut game_container = GameContainer::new(game_id);
        game_container.undo_stack.push(game.clone());
        game_map.insert(game_id.to_owned(), Arc::new(RwLock::new(game_container)));

        Ok(())
    }

    fn new(game_id: &str) -> Self {
        Self {
            game_id: game_id.to_string(),
            visitors: HashMap::new(),
            undo_stack: vec![],
            redo_stack: vec![],
        }
    }

    pub async fn add_user(game_id: &str, user_id: &str, name: &str) -> Result<(), GameError> {
        let game_map = GAME_MAP.read().await;
        let game = game_map.get(game_id);
        let game = match game {
            Some(g) => g,
            None => return Err(GameError::BadId(format!("{} does not exists", game_id))),
        };

        let user = LongPollUser::new(user_id, name);
        game.write()
            .await
            .visitors
            .insert(user_id.to_string(), user);

        Ok(())
    }

    pub async fn get_locked_container(
        game_id: &str,
    ) -> Result<Arc<RwLock<GameContainer>>, GameError> {
        let game_map = GAME_MAP.read().await; // Acquire read lock
        match game_map.get(game_id) {
            Some(container) => Ok(container.clone()),
            None => Err(GameError::BadId(format!("{} does not exist", game_id))),
        }
    }
    /**
     *  the game_handler will call this for the /longpoll url when there is a game id
     *  look up the LongPollUser and wait on the tokio::mcsp channel.  if there are no messages, we will wait until there
     *  is one.  if there are messages, we return the message and then come back for more.
     *
     *  will return an error if the inputs are bad
     */
    pub async fn wait_for_change(game_id: &str, user_id: &str) -> Result<CatanMessage, GameError> {
        trace_function!("wait_for_change", "game_id: {}", game_id);

        let user_rx = {
            let game_container_arc = Self::get_locked_container(game_id).await?;
            let game_container = game_container_arc.read().await;
            match game_container.visitors.get(user_id) {
                Some(u) => u.rx.clone(),
                None => return Err(GameError::BadId(format!("{} does not exist", user_id))),
            }
        };

        // Access the rx directly without locking
        let mut rx = user_rx.write().await;
        match rx.recv().await {
            Some(msg) => Ok(msg),
            None => Err(GameError::ChannelError(format!(
                "error writing channel. [user_id={}]",
                user_id
            ))),
        }
    }

    /**
     *  add a player to a game.  takes a write lock and writes a PlayerAdded message to the long poller.  while we have a
     *  game we could return (which has the players), at this point, I think the UI would be a "Create Game" UI where invites
     *  have been sent out and the UI reflects updates based on Accept/Reject
     */
    pub async fn add_player(game_id: &str, client_user: &ClientUser) -> Result<(), GameError> {
        trace_function!(
            "add_player",
            "game_id: {}, user_id: {}",
            game_id,
            client_user.id
        );

        let game_container = Self::get_locked_container(&game_id).await?;
        let mut game_container = game_container.write().await; // drop locked container
        if game_container.visitors.contains_key(&client_user.id) {
            return Err(GameError::AlreadyExists(format!(
                "{} already exists",
                client_user.id
            )));
        }
        let visitor = LongPollUser::new(&client_user.id, &client_user.user_profile.display_name);
        game_container
            .visitors
            .insert(client_user.id.to_string(), visitor);
        let game = game_container.undo_stack.last().clone().unwrap(); // you cannot have an empty undo stack *and a valid game_id
        let clone = game.add_user(client_user)?;
        game_container.undo_stack.push(clone.clone());

        drop(game_container);

        Self::broadcast_message(game_id, &CatanMessage::GameUpdate(clone)).await?;
        //
        //  count the clones!
        //  1. inside of add_user so that we can get a different instance for the undo_stack
        //  2. once to push onto the undo_stack
        //  3. one for each of the channels we send to
        //  the only time we don't clone is when we call broadcast (it is the instance returned by add_user)
        //
        Ok(())
    }
    /**
     *  send the message to all players in game_id
     */
    pub async fn broadcast_message(game_id: &str, message: &CatanMessage) -> Result<(), GameError> {
        trace_function!("broadcast_message", "game_id: {}", game_id);

        let game_container = Self::get_locked_container(&game_id).await?;
        let game_container = game_container.read().await; // drop locked container

        for v in game_container.visitors.values() {
            let res = v.tx.send(message.clone()).await;
            if res.is_err() {
                error!(
                    "ts.send failed [game_id={}][user_id:{}][name={}]",
                    game_id, v.user_id, v.name
                );
            }
        }

        Ok(())
    }

    pub async fn undo(game_id: &String) -> Result<(), GameError> {
        let game_container = Self::get_locked_container(game_id).await?;
        let mut game_container = game_container.write().await;
        let len = game_container.undo_stack.len();
        if len < 2 {
            return Err(GameError::ActionError(format!(
                "cannot undo first game. undo_stack len {} ",
                len
            )));
        }
        if !game_container.undo_stack.last().unwrap().can_undo {
            return Err(GameError::ActionError(
                "item in undo stack cannot be undone".to_string(),
            ));
        }
        let game = game_container.undo_stack.pop().unwrap();

        game_container.redo_stack.push(game.clone());
        Self::broadcast_message(game_id, &CatanMessage::GameUpdate(game)).await?;
        Ok(())
    }

    pub async fn current(game_id: &String) -> Result<RegularGame, GameError> {
        match Self::get_locked_container(game_id).await {
            Ok(game_container) => {
                let ro_container = game_container.read().await;
                Ok(ro_container.undo_stack.last().unwrap().clone())
            }
            Err(_) => Err(GameError::BadId(format!("{} not found", game_id))),
        }
    }

    pub async fn push_game(game_id: &String, game: &RegularGame) -> Result<(), GameError> {
        let game_container = Self::get_locked_container(game_id).await?;
        let mut rw_game_container = game_container.write().await;
        let game_clone = game.clone();
        rw_game_container.undo_stack.push(game_clone);
        rw_game_container.redo_stack.clear();
        drop(rw_game_container);
        Self::broadcast_message(game_id, &CatanMessage::GameUpdate(game.clone())).await?;
        Ok(())
    }
}
/**
 *  a GET that is a long polling get.  the call waits here until the game changes and then the service will signal
 *  and the call will complete, returning a CatanMessage.  the if GAME_HEADER is missing or "", then we longpoll
 *  for the LOBBY, otherwise send them for game updates.
 */
pub async fn long_poll_handler(req: HttpRequest) -> HttpResponse {
    full_info!("long_poll_handler called");
    defer!(full_info!("long_poll handler exited"));

    let user_id = req
        .headers()
        .get(GameHeader::USER_ID)
        .expect("should be added by auth mw")
        .to_str()
        .unwrap();

    let game_id_header = req.headers().get(GameHeader::GAME_ID);

    let message = if let Some(game_id) = game_id_header {
        let game_id_str = game_id.to_str().unwrap();
        if !game_id_str.is_empty() {
            GameContainer::wait_for_change(game_id_str, user_id).await
        } else {
            Lobby::wait_in_lobby(user_id).await
        }
    } else {
        Lobby::wait_in_lobby(user_id).await
    };

    HttpResponse::Ok()
        .content_type("application/json")
        .json(message)
}
