#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_assignments)]
#![allow(unreachable_code)]

use std::time::Duration;

use crate::games_service::game_container::game_messages::InvitationResponseData;
use crate::middleware::request_context_mw::TestContext;
use crate::{crack_game_update, wait_for_message};
use crate::{
    games_service::game_container::game_messages::CatanMessage, log_thread_info,
    shared::shared_models::UserProfile, trace_thread_info,
};
use crate::{shared::proxy::ServiceProxy, test::test_structs::HOST_URL};

use tokio::{sync::mpsc::Receiver, time::sleep};

use super::test_structs::ClientThreadHandler;
pub(crate) struct Handler1;
impl ClientThreadHandler for Handler1 {
    fn run(
        &self,
        rx: Receiver<CatanMessage>,
    ) -> std::pin::Pin<Box<dyn futures::Future<Output = ()> + Send>> {
        Box::pin(client1_thread(rx))
    }
}
pub(crate) async fn client1_thread(mut rx: Receiver<CatanMessage>) {
    let proxy = ServiceProxy::new(
        "james@longshotdev.com",
        "password", Some(TestContext::new(false, None)),
        HOST_URL,
    )
    .await
    .expect("Loging to work");
   
    let name = "James";

    let my_info: UserProfile = proxy
        .get_profile("Self")
        .await
        .to_profile()
        .expect("Successful call to get_profile should have a ClientUser in the body");
    trace_thread_info!(name, "Waiting for 500ms");
    tokio::time::sleep(Duration::from_millis(500)).await;
    trace_thread_info!(
        name,
        "Client thread. Waiting on Start Message from poll thread"
    );
    let message = wait_for_message!(name, rx);

    let game_id;

    //
    //  in the browser app, the browser worker is up and running before the UI, so you don't
    //  need to worry the issue of the main thread running before the polling threads. here
    //  we do -- so we just go to sleep for a bit.
    trace_thread_info!(name, "Sleeping for 1 second...");
    sleep(Duration::from_secs(1)).await;
    trace_thread_info!(name, "Game Thread Woke up!");

    let message = wait_for_message!(name, rx);
    if let CatanMessage::Invite(invite) = message.clone() {
        let response = InvitationResponseData::from_invitation(true, &invite);
        proxy
            .invitation_response(&response)
            .await
            .assert_success("accept invite should succeed)");
        game_id = invite.game_id.clone();
    } else {
        trace_thread_info!(name, "Wrong message received: {:?}", message);
    }

    let message = wait_for_message!(name, rx);

    assert!(
        matches!(message, CatanMessage::GameUpdate(_)),
        "Expected GameUpdate variant, got {:?}",
        message
    );

    let game = crack_game_update!(message).expect("Should be a GameUpdate!");
    assert_eq!(game.players.len(), 3);
    log_thread_info!(name, "end of test");
}
