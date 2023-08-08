#![allow(dead_code)]
#![allow(unused_variables)]

use crate::{
    games_service::game_container::game_messages::{CatanMessage, Invitation},
    log_thread_info,
    shared::models::ClientUser,
    trace_thread_info, wait_for_message,
};
use crate::{
    games_service::shared::game_enums::CatanGames, shared::proxy::ServiceProxy,
    test::test_structs::HOST_URL,
};
use futures::Future;
use std::{pin::Pin, time::Duration};
use tokio::{sync::mpsc::Receiver, time::sleep};

use super::test_structs::ClientThreadHandler;

pub(crate) struct Handler0;
impl ClientThreadHandler for Handler0 {
    fn run(&self, rx: Receiver<CatanMessage>) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        Box::pin(client0_thread(rx))
    }
}

// You can have Handler1, Handler2, etc., each implementing ClientThreadHandler.

/**
 *  this needs to create the game and send out invites.  once the game is started, then it is the same as any other
 *  thread
 */
pub(crate) async fn client0_thread(mut rx: Receiver<CatanMessage>) {
    let proxy = ServiceProxy::new(true, HOST_URL);
    let auth_token = proxy
        .get_authtoken("joe@longshotdev.com", "password")
        .await
        .expect("login should work");

    let name = "Main(Joe)";

    let my_info: ClientUser = proxy
        .get_profile(&auth_token)
        .await
        .expect("Unable to get profile")
        .json()
        .await
        .expect("get_profile should return a ClientUser");

    trace_thread_info!(name, "Waiting for 500ms");
    tokio::time::sleep(Duration::from_millis(500)).await;
    let message = wait_for_message!(name, rx);

    let game_id;

    //
    //  in the browser app, the browser worker is up and running before the UI, so you don't
    //  need to worry the issue of the main thread running before the polling threads. here
    //  we do -- so we just go to sleep for a bit.
    trace_thread_info!(name, "Sleeping for 1 second...");
    sleep(Duration::from_secs(1)).await;
    trace_thread_info!(name, "Game Thread Woke up!");

    trace_thread_info!(name, "creating new game");
    let response = proxy
        .new_game(CatanGames::Regular, &auth_token)
        .await
        .unwrap();
    assert!(
        response.status().is_success(),
        "error loggin in user: {}, err: {:#?}",
        my_info.user_profile.display_name,
        response
    );
    loop {
        let message = wait_for_message!(name, rx);
        if let CatanMessage::GameCreated(game) = message.clone() {
            game_id = game.game_id;
            break;
        } else {
            trace_thread_info!(name, "Wrong message received: {:?}", message);
        }
    }

    //
    // get the lobby
    trace_thread_info!(name, "Getting Lobby.");
    let res = proxy.get_lobby(&auth_token).await;

    let lobby = match res {
        Ok(response) => response
            .json::<Vec<ClientUser>>()
            .await
            .expect("should deserialize"),
        Err(e) => panic!("Error from get_lobby: {:?}", e),
    };

    trace_thread_info!(name, "get_lobby returned: {:#?}", lobby);

    for lobby_user in lobby {
        let my_clone = my_info.clone();
        let profile = my_clone.user_profile;
        if lobby_user.id == my_clone.id {
            continue; // don't invite myself
        }
        let invite_message = "Join my game!".to_string();
        let invitation = Invitation {
            from_id: my_clone.id.clone(),
            from_name: profile.display_name.clone(),
            to_id: lobby_user.id.clone(),
            to_name: lobby_user.user_profile.display_name.clone(),
            message: invite_message.clone(),
            from_picture: profile.picture_url.clone(),
            game_id: game_id.to_owned(),
        };
        trace_thread_info!(name, "Sending GameInvite");
        let response = proxy
            .send_invite(&invitation, &auth_token)
            .await
            .expect("send_invite should not fail");
    }

    let mut invited_players = Vec::new();
    loop {
        let message = wait_for_message!(name, rx);
        if let CatanMessage::InvitationResponse(invite_response) = message.clone() {
            assert!(invite_response.accepted, "invite should be accepted");
            trace_thread_info!(name, "players accepted: {}", invite_response.from_name);
            invited_players.push(invite_response.from_id);
            if invited_players.len() == 2 {
                break;
            }
        } else {
            trace_thread_info!(name, "Wrong message received: {:?}", message);
        }
    }
    trace_thread_info!(name, "all players accepted: {:#?}", players);

    log_thread_info!(name, "finished invitations.");

    //
    //  next tell service that all the players are added and we can start the game
    trace_thread_info!(name, "end of test:");
    // next we start the game
}
