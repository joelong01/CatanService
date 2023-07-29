#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, oneshot, RwLock};

use crate::games_service::game_container::game_messages::{CatanMessage, ErrorData, InviteData};

lazy_static::lazy_static! {
    // Initialize singleton lobby instance
    static ref LOBBY: Arc<RwLock<Lobby>> = Arc::new(RwLock::new(Lobby::new()));
}

#[derive(Debug)]
pub struct LobbyVisitor {
    pub user_id: String,
    pub notify: RwLock<Option<oneshot::Sender<CatanMessage>>>,
}

impl LobbyVisitor {
    pub fn new(user_id: String) -> Self {
        Self {
            user_id,
            notify: RwLock::new(None),
        }
    }
}

pub struct Lobby {
    visitors: HashMap<String, LobbyVisitor>,
    notify: broadcast::Sender<CatanMessage>,
}

impl Lobby {
    /// Create a new Lobby
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(1000);
        Self {
            visitors: HashMap::new(),
            notify: tx,
        }
    }

    /// Adds a new LobbyVisitor to the lobby if not already present.
    pub async fn join_lobby(user_id: String) {
        let mut lobby = LOBBY.write().await;
        let visitor = LobbyVisitor::new(user_id.clone());
        lobby.visitors.insert(user_id, visitor);
    }

    /// Removes a LobbyVisitor from the lobby, if present.
    pub async fn leave_lobby(user_id: &str) {
        let mut lobby = LOBBY.write().await;
        lobby.visitors.remove(user_id);
    }

    /// Returns a list of user_ids in the current lobby.
    pub async fn copy_lobby() -> Vec<String> {
        let lobby = LOBBY.read().await;
        lobby.visitors.keys().cloned().collect()
    }

    /// Awaits for an invitation message.
    pub async fn wait_for_invite(user_id: &str) -> CatanMessage {
        let lobby = LOBBY.read().await;
        if let Some(visitor) = lobby.visitors.get(user_id) {
            let (tx, rx) = oneshot::channel();
            let mut notify = visitor.notify.write().await;
            *notify = Some(tx);
            drop(notify);
            match rx.await {
                Ok(msg) => msg,
                Err(_) => CatanMessage::Error(ErrorData {
                    status_code: 500,
                    message: "Oneshot channel was dropped".to_string(),
                }),
            }
        } else {
            CatanMessage::Error(ErrorData {
                status_code: 404,
                message: "User not found in the lobby".to_string(),
            })
        }
    }

    /// Sends an invitation and notifies the recipient via the one-shot channel.
    pub async fn send_invite(from_id: &str, to_id: &str) -> Result<(), String> {
        let lobby = LOBBY.read().await;
        if let Some(visitor) = lobby.visitors.get(to_id) {
            let invite_data = InviteData {
                from: from_id.to_string(),
                to: to_id.to_string(),
            };
            let mut notify = visitor.notify.write().await;
            if let Some(tx) = notify.take() {
                tx.send(CatanMessage::Invite(invite_data))
                    .unwrap_or_else(|_| {
                        println!("Failed to send the invite");
                    });
                Ok(())
            } else {
                Err("No notification channel found for the user".into())
            }
        } else {
            Err("User not found in the lobby".into())
        }
    }
}
#[cfg(test)]
mod tests {
    use super::Lobby; // Assuming the test is in the same module as the Lobby implementation
    use crate::games_service::game_container::game_messages::{CatanMessage, InviteData};
    use std::sync::Arc;
    use log::info;
    use tokio::sync::Barrier;


    #[tokio::test]
    async fn test_lobby_invite_flow() {
        env_logger::init();
        info!("starting test_lobby_invite_flow");
        // 1. Create a lobby, add 2 users to it
        Lobby::join_lobby("user1".to_string()).await;
        Lobby::join_lobby("user2".to_string()).await;

        let barrier = Arc::new(Barrier::new(3));

        // Signal to synchronize tasks
        let barrier_clone = barrier.clone();
        let user1_wait = tokio::spawn(async move {
            info!("first thread started.  calling barrier_clone().wait().await");
            barrier_clone.wait().await; // barrier at 3
            loop {
                match Lobby::wait_for_invite("user1").await {
                    CatanMessage::Invite(invite_data) => {
                        if invite_data.from == "user3" {
                            break CatanMessage::Invite(invite_data);
                        }
                    }
                    _ => continue,
                }
            }
        });

        let barrier_clone = barrier.clone();
        let user2_wait = tokio::spawn(async move {
            info!("second thread started.  calling barrier_clone().wait().await");
            barrier_clone.wait().await; // barrier at 2
            loop {
                match Lobby::wait_for_invite("user2").await {
                    CatanMessage::Invite(invite_data) => {
                        if invite_data.from == "user3" {
                            break CatanMessage::Invite(invite_data);
                        }
                    }
                    _ => continue,
                }
            }
        });

        info!("first wait on main thread");
        barrier.wait().await; // Wait for the main task
        info!("through the barrier");

   

        // 3. Signal one user and ensure that it wakes up and returns the correct value; the other does not
        Lobby::send_invite("user3", "user1").await.unwrap();
        Lobby::send_invite("user3", "user2").await.unwrap(); // Simulate success

        let user1_result = user1_wait.await.unwrap();
        assert_eq!(
            user1_result,
            CatanMessage::Invite(InviteData {
                from: "user3".to_string(),
                to: "user1".to_string()
            })
        );

        let user2_result = user2_wait.await.unwrap();
        assert_eq!(
            user2_result,
            CatanMessage::Invite(InviteData {
                from: "user3".to_string(),
                to: "user2".to_string()
            })
        );
    }
}
