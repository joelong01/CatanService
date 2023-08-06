#![allow(dead_code)]
use std::pin::Pin;

use futures::Future;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::Receiver;

use crate::{
    games_service::game_container::game_messages::CatanMessage, shared::models::UserProfile
};

pub const HOST_URL: &str = "http://localhost:8082";

#[derive(Clone, Serialize, Deserialize, Eq, PartialEq)]
pub(crate) struct UserInfo {
    auth_token: String,
    user_profile: UserProfile,
    client_id: String,
}

#[derive(Clone)]
pub(crate) struct ClientData {
    pub auth_token: String,
    pub user_profile: UserProfile,
    pub client_id: String,
}

impl ClientData {
    pub(crate) fn new(client_id: &str, user_profile: &UserProfile, auth_token: &str) -> Self {
        Self {
            auth_token: auth_token.to_string(),
            user_profile: user_profile.clone(),
            client_id: client_id.to_string(),
        }
    }
}
// Define a trait with an async method matching the signature of your functions.
pub(crate) trait ClientThreadHandler: Send + Sync {
    fn run(&self, rx: Receiver<CatanMessage>) -> Pin<Box<dyn Future<Output = ()> + Send>>;
}

pub(crate) type ClientThread = fn(rx: &mut Receiver<CatanMessage>);

type AsyncClientThread = fn(rx: Receiver<CatanMessage>) -> Pin<Box<dyn Future<Output = ()> + Send>>;
#[macro_export]
macro_rules! wait_for_message {
    ($name:expr, $rx:expr) => {{
        thread_info!($name, "Begin wait for message");
        let message = $rx
            .recv()
            .await
            .unwrap_or_else(|| panic!("failed to receive message"));
        thread_info!($name, "received message: {:?}", message);
        message
    }};
}

