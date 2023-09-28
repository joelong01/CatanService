#![allow(dead_code)]
use crate::{
    games_service::game_container::game_messages::CatanMessage,
    middleware::request_context_mw::TestContext,
    shared::{proxy::ServiceProxy, shared_models::UserProfile},
    test::test_structs::HOST_URL,
    trace_thread_info,
};
pub async fn game_poller(username: &str, tx: tokio::sync::mpsc::Sender<CatanMessage>) {
    let proxy = ServiceProxy::new(
        username,
        "password",
        Some(TestContext::new(false, None, None)),
        HOST_URL,
    ).await.expect("login needs to work for test to run!");
   
    let client_user_profile: UserProfile = proxy
        .get_profile("Self")
        .await
        .to_profile()
        .expect("Client User should deserialize");
    // Create the client inside the spawned task
    let name = &client_user_profile.display_name;
    trace_thread_info!(name, "polling thread started, sending start message");
    tx.send(CatanMessage::Started(format!("{}  started", name)))
        .await
        .unwrap();
    trace_thread_info!(name, "returning from send start message");

    let mut game_id = "".to_string();
    let mut index = 0;
    loop {
        trace_thread_info!(name, "Begin poll. GameId: {}", game_id);

        let message = proxy
            .long_poll(&game_id, index)
            .await
            .get_service_message()
            .expect("long_poll should have a message in the body");

        trace_thread_info!(name, "long_poll returned: {:?}", message);
        if let CatanMessage::GameCreated(data) = message.clone() {
            game_id = data.game_id.clone()
        }

        if let CatanMessage::GameUpdate(data) = message.clone() {
            game_id = data.game_id.clone();
            index = data.game_index;
        }

        trace_thread_info!(name, "sending message: {:#?}", message);
        if let Err(e) = tx.send(message.clone()).await {
            trace_thread_info!(name, "Failed to send message: {}", e);
        }
    }
}
