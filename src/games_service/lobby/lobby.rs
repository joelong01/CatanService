#![allow(dead_code)]

use scopeguard::defer;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::{
    games_service::game_container::game_messages::{
        CatanMessage, GameCreatedData, LobbyUser,
    },
    thread_info, trace_function, shared::models::{LongPollUser, GameError},
};

lazy_static::lazy_static! {
    // Initialize singleton lobby instance
    static ref LOBBY: Arc<RwLock<Lobby>> = Arc::new(RwLock::new(Lobby::new()));
}



pub struct Lobby {
    visitors: HashMap<String, LongPollUser>,
  
}

impl Lobby {
    /// Create a new Lobby
    pub fn new() -> Self {

        Self {
            visitors: HashMap::new(),
        }
    }

    /// Adds a new LobbyVisitor to the lobby if not already present.
    pub async fn join_lobby(user_id: &str, name: &str) {
        let mut lobby = LOBBY.write().await;
        let visitor = LongPollUser::new(user_id, name);
        lobby.visitors.insert(user_id.into(), visitor);
    }

    /// Removes a LobbyVisitor from the lobby, if present.
    pub async fn leave_lobby(user_id: &str) {
        trace_function!("leave_lobby", "user_id: {}", user_id);
        let mut lobby = LOBBY.write().await;
        lobby.visitors.remove(user_id);
    }

    /// Returns a list of user_ids in the current lobby.
    pub async fn copy_lobby() -> Vec<LobbyUser> {
        let lobby = LOBBY.read().await;
        let mut users = Vec::new();
        for v in lobby.visitors.values() {
            let user = LobbyUser {
                user_id: v.user_id.clone(),
                user_name: v.name.clone(),
            };
            users.push(user);
        }

        users
    }
    //
    //  send a message to the lobby waiter that a game has been created.  the client should get this and use
    //  the game_id to wait in the GameContainer
    pub async fn game_created(game_id: &str, user_id: &str) -> Result<(), String> {
        trace_function!("game_created", "user_id: {}", user_id);
        let msg = GameCreatedData {
            user_id: user_id.to_owned(),
            game_id: game_id.to_owned(),
        };
        match Lobby::send_message(user_id, &CatanMessage::GameCreated(msg)).await {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("failed to send message in game_created {:?}", e)),
        }
    }
    /// Awaits for an message.
    pub async fn wait_in_lobby(user_id: &str) -> Result<CatanMessage, GameError> {
        trace_function!("wait_in_lobby", "user_id: {}", user_id);
        let rx = {
            let ro_lobby = LOBBY.read().await;
            if let Some(visitor) = ro_lobby.visitors.get(user_id) {
                visitor.rx.clone() // clone the Ar<RwLock>, not the rx
            } else {
                return Err(GameError::BadId(format!("{} not found", user_id)));
            }
        };
    
        let mut rx_lock = rx.write().await;
    
        // Now the read lock and Mutex lock are dropped, and you can await the message
        match rx_lock.recv().await {
            Some(msg) => Ok(msg),
            None => {
                thread_info!(
                    "wait_in_loby",
                    "ERROR: recv() for failed to return data. user_id: {}",
                    user_id,
                );
                return Err(GameError::ChannelError("Error in wait_in_lobby in recv".to_string()));
            }
        }
    }
    
    pub async fn send_message(to_id: &str, message: &CatanMessage) -> Result<(), String> {
        trace_function!("send_message", "to: {}, message: {:?}", to_id, message);
        let lobby = LOBBY.read().await;
        if let Some(visitor) = lobby.visitors.get(to_id) {
            visitor.tx.send(message.clone()).await.map_err(|e| e.to_string())
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
        Lobby::join_lobby("user1", "user1").await;
        Lobby::join_lobby("user2", "user2").await;

        let barrier = Arc::new(Barrier::new(3));
        let message = "Join my game!";
        let picture_url = "https://example.com/pic.jpg";

        // Signal to synchronize tasks
        let barrier_clone = barrier.clone();
        let user1_wait = tokio::spawn(async move {
            full_info!("first thread started.  calling barrier_clone().wait().await");
            barrier_clone.wait().await; // barrier at 3
            loop {
                match Lobby::wait_in_lobby("user1").await {
                    Ok(CatanMessage::Invite(invite_data)) => {
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
                match Lobby::wait_in_lobby("user2").await.expect("no errors!") {
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
        const USER1_ID: &str = "user1";
        const USER2_ID: &str = "user2";
        const USER3_ID: &str = "user3";
        let invitation_to_user1 = Invitation {
            from_id: USER3_ID.into(),
            to_id: USER1_ID.into(),
            from_name: "User3".into(),
            to_name: "User1".into(),
            message: message.clone().into(),
            from_picture: picture_url.into(),
            game_id: "TODO!".to_owned(),
        };
        let invitation_to_user2 = Invitation {
            from_id: USER3_ID.into(),
            to_id: USER2_ID.into(),
            from_name: "User3".into(),
            to_name: "User2".into(),
            message: message.into(),
            from_picture: picture_url.into(),
            game_id: "TODO!".to_owned(),
        };

        Lobby::send_message(USER1_ID, &CatanMessage::Invite(invitation_to_user1.clone()))
            .await
            .unwrap();
        Lobby::send_message(USER2_ID, &CatanMessage::Invite(invitation_to_user2.clone()))
            .await
            .unwrap(); // Simulate success

        let negative_test_invite = Invitation {
            from_id: "user30".into(),
            to_id: "user20".into(),
            from_name: "user30".into(),
            to_name: "user20".into(),
            message: message.into(),
            from_picture: picture_url.into(),
            game_id: "TODO!".to_owned(),
        };

        // send an invite to a non-existing user
        assert!(
            !Lobby::send_message("user30", &CatanMessage::Invite(negative_test_invite))
                .await
                .is_ok()
        ); //

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
        let user_id = "test_lobby_join_leave_flow:user1";

        // Attempt to join the lobby
        Lobby::join_lobby(user_id, user_id).await;
        let lobby_after_join = Lobby::copy_lobby().await;
        assert!(
            lobby_after_join
                .iter()
                .any(|user| user.user_id == user_id.to_string()),
            "Lobby should contain user after joining"
        );

        // Attempt to join the lobby again (double join)
        Lobby::join_lobby(user_id, user_id).await;
        let lobby_after_double_join = Lobby::copy_lobby().await;
        assert!(
            lobby_after_join
                .iter()
                .any(|user| user.user_id == user_id.to_string()),
            "Lobby should contain user after joining"
        );

        assert_eq!(
            lobby_after_double_join
                .iter()
                .filter(|user| user.user_id == user_id.to_string())
                .count(),
            1,
            "Lobby should contain exactly one instance of the user ID after double joining"
        );

        // Attempt to leave the lobby
        Lobby::leave_lobby(&user_id).await;
        let lobby_after_leave = Lobby::copy_lobby().await;
        assert!(
            !lobby_after_leave
                .iter()
                .any(|user| user.user_id == user_id.to_string()),
            "Lobby should not contain user after leaving"
        );

        // Attempt to leave the lobby again (double leave)
        Lobby::leave_lobby(&user_id).await;
        let lobby_after_double_leave = Lobby::copy_lobby().await;
        assert!(
            !lobby_after_double_leave
                .iter()
                .any(|user| user.user_id == user_id.to_string()),
            "Lobby should still not contain user after double leaving"
        );
    }
}
