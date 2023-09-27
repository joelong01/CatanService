#![allow(dead_code)]
#![allow(unused_variables)]

use crate::{
    crack_game_created, crack_game_update,
    games_service::{
        catan_games::games::regular::regular_game::RegularGame,
        game_container::game_messages::{CatanMessage, Invitation},
        shared::game_enums::GameAction,
    },
    log_thread_info,
    middleware::request_context_mw::TestContext,
    shared::shared_models::UserProfile,
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
    let proxy = ServiceProxy::new(
        "joe@longshotdev.com",
        "password",
        Some(TestContext::new(false, None, None)),
        HOST_URL,
    ).await.expect("login to succeed");

    let name = "Main(Joe)";

    let my_info: UserProfile = proxy
        .get_profile("Self")
        .await
        .to_profile()
        .expect("Successful call to get_profile should have a ClientUser in the body");

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

    let test_game = load_game().expect(&format!("Test game should be in {}", TEST_GAME_LOC));
    let returned_game = proxy
        .new_game(CatanGames::Regular, Some(&test_game))
        .await
        .get_game()
        .expect("Should have a RegularGame returned in the body");

    let message = wait_for_message!(name, rx);
    let game_created = crack_game_created!(message).expect("should be a game!");
    game_id = game_created.game_id;
    assert_eq!(game_id, test_game.id);

    //
    // get the lobby
    trace_thread_info!(name, "Getting Lobby.");
    let lobby = proxy
        .get_lobby()
        .await
        .get_profile_vec()
        .expect("Vec<> should be in body");

    trace_thread_info!(name, "get_lobby returned: {:#?}", lobby);

    for lobby_user in lobby {
        let cloned_profile = my_info.clone();

        if lobby_user.user_id == cloned_profile.user_id {
            continue; // don't invite myself
        }
        let invite_message = "Join my game!".to_string();
        let invitation = Invitation {
            from_id: cloned_profile.user_id.clone().unwrap(),
            from_name: cloned_profile.display_name.clone(),
            to_id: lobby_user.user_id.clone().unwrap(),
            to_name: lobby_user.display_name.clone(),
            message: invite_message.clone(),
            from_picture: cloned_profile.picture_url.clone(),
            game_id: game_id.to_owned(),
        };
        trace_thread_info!(name, "Sending GameInvite");
        let response = proxy
            .send_invite(&invitation)
            .await
            .assert_success("send_invite should not fail");
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

        let actions = proxy
            .get_actions(&game_id)
            .await
            .assert_success("get actions to succeed")
            .get_actions()
            .expect("get actions should have a Vec of valid actions in the body");
        assert!(actions.len() == 2);
        assert!(actions.contains(&GameAction::AddPlayer));

        assert!(actions.contains(&GameAction::Next));
    }
    trace_thread_info!(name, "all players accepted: {:#?}", invited_players);

    proxy
        .start_game(&game_id)
        .await
        .assert_success("start should not fail");
    let message = wait_for_message!(name, rx);

    assert!(
        matches!(message, CatanMessage::GameUpdate(_)),
        "Expected GameUpdate variant, got {:?}",
        message
    );

    let game = crack_game_update!(message).expect("Should be a GameUpdate!");
    assert_eq!(game.players.len(), 3);

    // what actions can I take?

    let actions = proxy
        .get_actions(&game_id)
        .await
        .assert_success("get actions to succeed")
        .get_actions()
        .expect("get_actions to have a Vec<GameAction> in the body");

    assert!(actions.len() == 2);

    log_thread_info!(name, "end of test");
    // next we start the game
}
pub const TEST_GAME_LOC: &'static str = "./src/test/test_game.json";

pub fn save_game(game: &RegularGame) {
    assert_eq!(game.players.len(), 1); // needed for the new_game logic
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(TEST_GAME_LOC)
        .unwrap();

    // Write the JSON string to the file
    std::io::Write::write_all(
        &mut file,
        serde_json::to_string_pretty(game).unwrap().as_bytes(),
    )
    .unwrap();
}

pub fn load_game() -> Result<RegularGame, Box<dyn std::error::Error>> {
    // Read the file to a string
    match std::fs::read_to_string(TEST_GAME_LOC) {
        Ok(contents) => {
            // Parse the string as JSON into a RegularGame object
            let game: RegularGame = match serde_json::from_str(&contents) {
                Ok(game) => game,
                Err(e) => {
                    log_thread_info!("load_game", "failed to parse game from JSON: {:#?}", e);
                    return Err(Box::new(e));
                }
            };

            Ok(game) // Return the Result with the RegularGame
        }
        Err(e) => {
            log_thread_info!(
                "load_game",
                "failed to load {}.  error: {:#?}",
                TEST_GAME_LOC,
                e
            );

            Err(Box::new(e)) // Convert the Error to a Box<dyn std::error::Error> and return it
        }
    }
}
