#![allow(dead_code)]
use reqwest::StatusCode;
use scopeguard::defer;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock};

use crate::{
    games_service::game_container::game_messages::{CatanMessage, GameStatus},
    log_thread_info,
    shared::models::{ClientUser, GameError, ResponseType, ServiceResponse, UserProfile},
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
    user_profile: UserProfile,
    pub tx: mpsc::Sender<ServiceResponse>,
    pub rx: Arc<Mutex<mpsc::Receiver<ServiceResponse>>>,
    pub status: GameStatus,
}

impl LongPoller {
    pub fn new(user_id: &str, profile: &UserProfile) -> Self {
        let (tx, rx) = mpsc::channel(0x64);
        Self {
            user_id: user_id.to_owned(),
            tx,
            rx: Arc::new(Mutex::new(rx)),
            status: GameStatus::Available,
            user_profile: profile.clone(),
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
    pub async fn add_user(user_id: &str, profile: &UserProfile) -> Result<(), GameError> {
        let mut users_map = ALL_USERS_MAP.write().await; // Acquire write lock
        if users_map.contains_key(user_id) {
            return Err(GameError::BadId(format!("{} already exists", user_id)));
        }
        users_map.insert(
            user_id.to_owned(),
            Arc::new(RwLock::new(LongPoller::new(user_id, profile))),
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
        message: &CatanMessage,
    ) -> Result<ServiceResponse, ServiceResponse> {
        log_thread_info!(
            "send_message",
            "enter [to:{:#?}] [message={:?}]",
            to_users,
            message
        );
        defer! {log_thread_info!("send_message","leave [to:{:#?}] [message={:?}]", to_users, message )};

        let users_map = ALL_USERS_MAP.read().await; // Acquire read lock

        let service_response = ServiceResponse::new(
            "",
            StatusCode::OK,
            ResponseType::ServiceMessage(message.clone()),
            GameError::NoError,
        );

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
            if tx.send(service_response.clone()).await.is_err() {
                errors.push((
                    to.clone(),
                    GameError::ChannelError(format!("error in tx.send for {}", to)),
                ));
            }
        }

        if errors.is_empty() {
            Ok(ServiceResponse::new(
                "",
                StatusCode::OK,
                ResponseType::NoData,
                GameError::NoError,
            ))
        } else {
            Err(ServiceResponse::new(
                "",
                StatusCode::INTERNAL_SERVER_ERROR,
                ResponseType::SendMessageError(errors),
                GameError::ChannelError(String::default()),
            ))
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

    pub async fn wait(user_id: &str) -> Result<ServiceResponse, ServiceResponse> {
        let user_rx = {
            let users_map = ALL_USERS_MAP.read().await;
            match users_map.get(user_id) {
                Some(lp) => lp.read().await.rx.clone(),
                None => return Err(ServiceResponse::new_bad_id("in long poller", user_id)),
            }
        };

        // Access the rx by taking a write lock -- this'd be bad if there were multipler readers, but our MEP says
        // we can only have one at a time, *and* so does our mpsc channel.
        //
        let mut rx = user_rx.lock().await;
        match rx.recv().await {
            Some(msg) => Ok(msg),
            None => Err(ServiceResponse::new(
                &format!("error writing channel. [user_id={}]", user_id),
                reqwest::StatusCode::INTERNAL_SERVER_ERROR,
                ResponseType::NoData,
                GameError::ChannelError(String::default()),
            )),
        }
    }
    /// returns all logged in users marked as "Available"
    ///
    /// # Arguments
    /// # Returns
    ///
    /// * a Vec us user_ids
    pub async fn get_available() -> Vec<ClientUser> {
        let mut available = Vec::new();
        let users = ALL_USERS_MAP.read().await;
        for u in users.values() {
            let lp = u.read().await;
            if lp.status == GameStatus::Available {
                available.push(ClientUser {
                    id: lp.user_id.clone(),
                    user_profile: lp.user_profile.clone(),
                });
            }
        }
        available
    }

    pub async fn set_status(user_id: &str, status: GameStatus) -> Result<(), GameError> {
        let users_map = ALL_USERS_MAP.write().await; // Acquire write lock

        if let Some(user) = users_map.get(user_id) {
            let mut lp = user.write().await;
            lp.status = status;
            Ok(())
        } else {
            Err(GameError::BadId(format!("{} does not exist", user_id)))
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_add_user() {
        assert_eq!(
            LongPoller::add_user("user1", &UserProfile::default()).await,
            Ok(())
        );
        assert_eq!(
            LongPoller::add_user("user1", &UserProfile::default()).await,
            Err(GameError::BadId(String::from("user1 already exists")))
        );
    }

    #[tokio::test]
    async fn test_remove_user() {
        assert_eq!(
            LongPoller::add_user("user2", &UserProfile::default()).await,
            Ok(())
        );
        assert_eq!(LongPoller::remove_user("user2").await, Ok(()));
        assert_eq!(
            LongPoller::remove_user("user2").await,
            Err(GameError::BadId(String::from("user2 does not exist")))
        );
    }

    #[tokio::test]
    async fn test_wait() {
        assert_eq!(
            LongPoller::add_user("user5", &UserProfile::default()).await,
            Ok(())
        );
        let message = CatanMessage::Started("".to_string());
        let message_clone = message.clone(); // Clone the message

        // Spawn a task to send the message after a delay
        tokio::spawn(async move {
            // Note the "move" keyword
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            LongPoller::send_message(vec!["user5".to_string()], &message_clone)
                .await
                .unwrap(); // Use the cloned message
        });

        assert_eq!(LongPoller::wait("user5").await.unwrap().get_service_message().unwrap(), message);
        assert!(LongPoller::wait("user6").await.is_err());
    }
    #[tokio::test]
    async fn test_get_available_and_set_status() {
        // Add users
        assert_eq!(
            LongPoller::add_user("user1", &UserProfile::default()).await,
            Ok(())
        );
        assert_eq!(
            LongPoller::add_user("user2", &UserProfile::default()).await,
            Ok(())
        );
        assert_eq!(
            LongPoller::add_user("user3", &UserProfile::default()).await,
            Ok(())
        );

        // Set status
        assert_eq!(
            LongPoller::set_status("user1", GameStatus::Available).await,
            Ok(())
        );
        assert_eq!(
            LongPoller::set_status("user2", GameStatus::PlayingGame).await,
            Ok(())
        );
        assert_eq!(
            LongPoller::set_status("user3", GameStatus::Available).await,
            Ok(())
        );

        // Test get_available
        let available_users = LongPoller::get_available().await;
        assert_eq!(available_users.len(), 2);
    }
}
