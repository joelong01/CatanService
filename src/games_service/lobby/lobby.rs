#![allow(dead_code)]

use scopeguard::defer;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, oneshot, RwLock};

use crate::{
    full_info,
    games_service::game_container::game_messages::{
        CatanMessage, ErrorData, GameCreatedData, Invitation,
    },
};

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
        full_info!("leave_lobby enter");
        defer! {full_info!("leave_lobby exit");}
        let mut lobby = LOBBY.write().await;
        lobby.visitors.remove(user_id);
    }

    /// Returns a list of user_ids in the current lobby.
    pub async fn copy_lobby() -> Vec<String> {
        let lobby = LOBBY.read().await;
        lobby.visitors.keys().cloned().collect()
    }
    //
    //  when a game is created, we pull them from the lobby and send a GameCreated message
    pub async fn game_created(game_id: &str, user_id: &str) -> Result<(), String> {
        let msg = GameCreatedData {
            user_id: user_id.to_owned(),
            game_id: game_id.to_owned(),
        };
        let lobby = LOBBY.read().await;
        if let Some(visitor) = lobby.visitors.get(user_id) {
            let mut notify = visitor.notify.write().await;
            if let Some(tx) = notify.take() {
                tx.send(CatanMessage::GameCreated(msg)).unwrap_or_else(|_| {
                    println!("Failed to send the GameCreated message");
                });
                Ok(())
            } else {
                Err("No notification channel found for the user".into())
            }
        } else {
            Err("User not found in the lobby".into())
        }
    }
    /// Awaits for an invitation message.
    pub async fn wait_for_invite(user_id: &str) -> CatanMessage {
        full_info!("long poll wait_for_invite");
        defer! {full_info!("leaving wait_for_invite");}
        let ro_lobby = LOBBY.read().await;
        if let Some(visitor) = ro_lobby.visitors.get(user_id) {
            let (tx, rx) = oneshot::channel();
            let mut rw_notify = visitor.notify.write().await;
            *rw_notify = Some(tx);
            drop(rw_notify);
            drop(ro_lobby);
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
    pub async fn send_invite(invitation: &Invitation) -> Result<(), String> {
        let lobby = LOBBY.read().await;
        if let Some(visitor) = lobby.visitors.get(&invitation.to_id) {
            let mut notify = visitor.notify.write().await;
            if let Some(tx) = notify.take() {
                tx.send(CatanMessage::Invite(invitation.clone()))
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
    use crate::{
        full_info,
        games_service::game_container::game_messages::{CatanMessage, Invitation},
    };
    use std::sync::Arc;
    use tokio::sync::Barrier;

    #[tokio::test]
    async fn test_lobby_invite_flow() {
        env_logger::try_init().ok();
        full_info!("starting test_lobby_invite_flow");
        // 1. Create a lobby, add 2 users to it
        Lobby::join_lobby("user1".to_string()).await;
        Lobby::join_lobby("user2".to_string()).await;

        let barrier = Arc::new(Barrier::new(3));
        let message = "Join my game!".to_string();
        let picture_url = "https://example.com/pic.jpg".to_string();

        // Signal to synchronize tasks
        let barrier_clone = barrier.clone();
        let user1_wait = tokio::spawn(async move {
            full_info!("first thread started.  calling barrier_clone().wait().await");
            barrier_clone.wait().await; // barrier at 3
            loop {
                match Lobby::wait_for_invite("user1").await {
                    CatanMessage::Invite(invite_data) => {
                        if invite_data.from_id == "user3" {
                            break CatanMessage::Invite(invite_data);
                        }
                    }
                    _ => continue,
                }
            }
        });

        let barrier_clone = barrier.clone();
        let user2_wait = tokio::spawn(async move {
            full_info!("second thread started.  calling barrier_clone().wait().await");
            barrier_clone.wait().await; // barrier at 2
            loop {
                match Lobby::wait_for_invite("user2").await {
                    CatanMessage::Invite(invite_data) => {
                        if invite_data.from_id == "user3" {
                            break CatanMessage::Invite(invite_data);
                        }
                    }
                    _ => continue,
                }
            }
        });

        full_info!("first wait on main thread");
        barrier.wait().await; // Wait for the main task
        full_info!("through the barrier");

        let invitation_to_user1 = Invitation {
            from_id: "user3".to_string(),
            to_id: "user1".to_string(),
            from_name: "User3".to_string(),
            message: message.clone(),
            picture_url: picture_url.clone(),
            game_id: "TODO!".to_owned(),
        };
        let invitation_to_user2 = Invitation {
            from_id: "user3".to_string(),
            to_id: "user2".to_string(),
            from_name: "User3".to_string(),
            message: message.clone(),
            picture_url: picture_url.clone(),
            game_id: "TODO!".to_owned(),
        };

        Lobby::send_invite(&invitation_to_user1).await.unwrap();
        Lobby::send_invite(&invitation_to_user2).await.unwrap(); // Simulate success

        let negative_test_invite = Invitation {
            from_id: "user30".to_string(),
            to_id: "user20".to_string(),
            from_name: "User3".to_string(),
            message: message.clone(),
            picture_url: picture_url.clone(),
            game_id: "TODO!".to_owned(),
        };

        // send an invite to a non-existing user
        assert!(!Lobby::send_invite(&negative_test_invite).await.is_ok()); //

        let user1_result = user1_wait.await.unwrap();
        assert_eq!(user1_result, CatanMessage::Invite(invitation_to_user1));

        let user2_result = user2_wait.await.unwrap();
        assert_eq!(user2_result, CatanMessage::Invite(invitation_to_user2));

        Lobby::leave_lobby("user1").await;
        Lobby::leave_lobby("user2").await;
        full_info!(
            "{}",
            serde_json::to_string(&Lobby::copy_lobby().await).unwrap()
        );
    }
    #[tokio::test]
    async fn test_lobby_join_leave_flow() {
        env_logger::try_init().ok();
        full_info!("starting test_lobby_join_leave_flow");

        // Define user ID specific to this test
        let user_id = "test_lobby_join_leave_flow:user1".to_string();

        // Attempt to join the lobby
        Lobby::join_lobby(user_id.clone()).await;
        let lobby_after_join = Lobby::copy_lobby().await;
        assert!(
            lobby_after_join.contains(&user_id),
            "Lobby should contain user after joining"
        );

        // Attempt to join the lobby again (double join)
        Lobby::join_lobby(user_id.clone()).await;
        let lobby_after_double_join = Lobby::copy_lobby().await;
        assert!(
            lobby_after_double_join.contains(&user_id),
            "Lobby should still contain user after double joining"
        );
        assert_eq!(
            lobby_after_double_join
                .iter()
                .filter(|&id| *id == user_id)
                .count(),
            1,
            "Lobby should contain exactly one instance of the user ID after double joining"
        );

        // Attempt to leave the lobby
        Lobby::leave_lobby(&user_id).await;
        let lobby_after_leave = Lobby::copy_lobby().await;
        assert!(
            !lobby_after_leave.contains(&user_id),
            "Lobby should not contain user after leaving"
        );

        // Attempt to leave the lobby again (double leave)
        Lobby::leave_lobby(&user_id).await;
        let lobby_after_double_leave = Lobby::copy_lobby().await;
        assert!(
            !lobby_after_double_leave.contains(&user_id),
            "Lobby should still not contain user after double leaving"
        );
    }
}
