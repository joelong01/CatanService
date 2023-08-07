#![allow(dead_code)]
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

use crate::{
    games_service::game_container::game_messages::CatanMessage, shared::models::GameError,
};
//
//  this is a map of "waiters" - holding all the state necessary for a Long Poller to wait on a thread
//  and other threads to find and call send on the tx
//
lazy_static::lazy_static! {
    static ref ALL_USERS_MAP: Arc<RwLock<HashMap<String, Arc<RwLock<LongPoller>>>>> = Arc::new(RwLock::new(HashMap::new()));
}

#[derive(Debug)]
pub struct LongPoller {
    user_id: String, // can be any kind of id
    pub tx: mpsc::Sender<CatanMessage>,
    pub rx: Arc<RwLock<mpsc::Receiver<CatanMessage>>>,
}

impl LongPoller {
    pub fn new(user_id: &str) -> Self {
        let (tx, rx) = mpsc::channel(0x64);
        Self {
            user_id: user_id.to_owned(),
            tx,
            rx: Arc::new(RwLock::new(rx)),
        }
    }
}
/**
 *  Add the user to the hashmap by puttin ghtem in a LongPoller struct
 */
pub async fn add_user(user_id: &str) -> Result<(), GameError> {
    let mut users_map = ALL_USERS_MAP.write().await; // Acquire write lock
    if users_map.contains_key(user_id) {
        return Err(GameError::BadId(format!("{} already exists", user_id)));
    }
    users_map.insert(
        user_id.to_owned(),
        Arc::new(RwLock::new(LongPoller::new(user_id))),
    );
    Ok(())
}

/**
 *  Removes the user from the map -- errors if they aren't there
 */
pub async fn remove_user(user_id: &str) -> Result<(), GameError> {
    let mut users_map = ALL_USERS_MAP.write().await; // Acquire write lock

    match users_map.remove(user_id) {
        Some(_) => Ok(()),
        None => Err(GameError::BadId(format!("{} does not exist", user_id))),
    }
}

pub async fn send_message(
    to_users: Vec<String>,
    message: CatanMessage,
) -> Result<(), Vec<(String, GameError)>> {
    let users_map = ALL_USERS_MAP.read().await; // Acquire read lock
    let mut results = Vec::<(String, GameError)>::new();
    for to in to_users {
        let user = users_map.get(&to);
        match user {
            Some(user) => {
                let lp = user.read().await;
                let res = lp.tx.send(message.clone()).await;
                if res.is_err() {
                    results.push((
                        lp.user_id.clone(),
                        GameError::ChannelError(format!("error in tx.send for {}", lp.user_id.clone())),
                    ))
                }
            }
            None => {
                    results.push((to.clone(), GameError::BadId(format!("id {} not in list", to))));
                }
        }
    }
    if results.len() == 0 {
        Ok(())
    } else {
        Err(results)
    }
}

pub async fn wait(user_id: &str) -> Result<CatanMessage, GameError> {

    let user_rx = {
        let users_map = ALL_USERS_MAP.read().await;
        match users_map.get(user_id) {
            Some(lp) => lp.read().await.rx.clone(),
            None => return Err(GameError::BadId(format!("{} does not exist", user_id))),
        }
    };

    // Access the rx by taking a write lock -- this'd be bad if there were multipler readers, but our MEP says
    // we can only have one at a time, *and* so does our mpsc channel.
    //
    let mut rx = user_rx.write().await;
    match rx.recv().await {
        Some(msg) => Ok(msg),
        None => Err(GameError::ChannelError(format!(
            "error writing channel. [user_id={}]",
            user_id
        ))),
    }
}
