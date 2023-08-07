#![allow(dead_code)]
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock};

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
    pub rx: Arc<Mutex<mpsc::Receiver<CatanMessage>>>,
}

impl LongPoller {
    pub fn new(user_id: &str) -> Self {
        let (tx, rx) = mpsc::channel(0x64);
        Self {
            user_id: user_id.to_owned(),
            tx,
            rx: Arc::new(Mutex::new(rx)),
        }
    }
}
/// Add the user to the hashmap by putting them in a LongPoller struct.
///
/// # Arguments
///
/// * `user_id` - A string slice reference containing the user ID.
///
/// # Returns
///
/// * `Result<(), GameError>` - Ok if the user was successfully added; Err with a GameError if the user already exists.
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

/// Removes the user from the map, returning an error if they aren't there.
///
/// # Arguments
///
/// * `user_id` - A string slice reference containing the user ID to be removed.
///
/// # Returns
///
/// * `Result<(), GameError>` - Ok if the user was successfully removed; Err with a GameError if the user does not exist.
pub async fn remove_user(user_id: &str) -> Result<(), GameError> {
    let mut users_map = ALL_USERS_MAP.write().await; // Acquire write lock

    match users_map.remove(user_id) {
        Some(_) => Ok(()),
        None => Err(GameError::BadId(format!("{} does not exist", user_id))),
    }
}
/// Sends a message to a list of users.
///
/// # Arguments
///
/// * `to_users` - A vector of user IDs (represented as Strings) to whom the message will be sent.
/// 
/// * `message` - The message to send, of type `CatanMessage`.
///
/// # Returns
///
/// * `Result<(), Vec<(String, GameError)>>` - Ok if the message was sent to all users; 
/// Err with a vector of tuples containing user IDs and the corresponding GameError for users to 
/// whom the message could not be sent.

pub async fn send_message(
    to_users: Vec<String>,
    message: CatanMessage,
) -> Result<(), Vec<(String, GameError)>> {
    let users_map = ALL_USERS_MAP.read().await; // Acquire read lock

    // Collect the senders and check for missing users
    let mut senders = Vec::new();
    let mut errors = Vec::new();
    for to in &to_users {
        match users_map.get(to) {
            Some(user) => {
                let lp = user.read().await;
                senders.push(lp.tx.clone());
            }
            None => {
                errors.push((
                    to.clone(),
                    GameError::BadId(format!("id {} not in list", to)),
                ));
            }
        }
    }
    drop(users_map); // Explicitly drop the read lock

    // Send the messages
    for (tx, to) in senders.into_iter().zip(to_users.iter()) {
        if tx.send(message.clone()).await.is_err() {
            errors.push((
                to.clone(),
                GameError::ChannelError(format!("error in tx.send for {}", to)),
            ));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
/// Waits for a message for the specified user ID.
///
/// # Arguments
///
/// * `user_id` - The user ID for which to wait for a message.
///
/// # Returns
///
/// * `Result<CatanMessage, GameError>` - Ok with the received message if successful; Err 
/// with GameError if the user does not exist or if there was an error reading the channel.
///
/// Note: This function takes a write lock to access the receiver, ensuring exclusive access. 
/// 
/// It is designed to be used with a model that allows only one reader at a time for the 
/// specified user ID.

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
    let mut rx = user_rx.lock().await;
    match rx.recv().await {
        Some(msg) => Ok(msg),
        None => Err(GameError::ChannelError(format!(
            "error writing channel. [user_id={}]",
            user_id
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_add_user() {
        assert_eq!(add_user("user1").await, Ok(()));
        assert_eq!(
            add_user("user1").await,
            Err(GameError::BadId(String::from("user1 already exists")))
        );
    }

    #[tokio::test]
    async fn test_remove_user() {
        assert_eq!(add_user("user2").await, Ok(()));
        assert_eq!(remove_user("user2").await, Ok(()));
        assert_eq!(
            remove_user("user2").await,
            Err(GameError::BadId(String::from("user2 does not exist")))
        );
    }

    #[tokio::test]
    async fn test_wait() {
        assert_eq!(add_user("user5").await, Ok(()));
        let message = CatanMessage::Started("".to_string());
        let message_clone = message.clone(); // Clone the message
    
        // Spawn a task to send the message after a delay
        tokio::spawn(async move { // Note the "move" keyword
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            send_message(vec!["user5".to_string()], message_clone).await.unwrap(); // Use the cloned message
        });
    
        assert_eq!(wait("user5").await, Ok(message));
        assert_eq!(
            wait("user6").await,
            Err(GameError::BadId(String::from("user6 does not exist")))
        );
    }
    
    

}