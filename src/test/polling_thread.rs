
use crate::{
    games_service::game_container::game_messages::CatanMessage,
    shared::{models::ClientUser, proxy::ServiceProxy},
    test::test_structs::HOST_URL,
    thread_info,
};

pub async fn polling_thread(username: &str, tx: tokio::sync::mpsc::Sender<CatanMessage>) {
    let proxy = ServiceProxy::new(true, HOST_URL);
    let auth_token = &proxy
        .get_authtoken(username, "password")
        .await
        .expect("Login should work!");

    let client_user: ClientUser = proxy
        .get_profile(&auth_token)
        .await
        .expect("get profile should work")
        .json()
        .await
        .expect("Client User should deserialize");
    // Create the client inside the spawned task
    let name = &client_user.user_profile.display_name;
    thread_info!(name, "polling thread started, sending start message");
    tx.send(CatanMessage::Started(format!("{}  started", name)))
        .await
        .unwrap();
    thread_info!(name, "returning from send start message");

    let mut game_id = "".to_string();

    loop {
        thread_info!(name, "Begin poll. GameId: {}", game_id);

        let response = proxy.long_poll(&game_id, auth_token).await.unwrap();
        assert!(
            response.status().is_success(),
            "error coming back from long_poll {:#?}",
            response
        );
        thread_info!(name, "poll returned. GameId: {}", game_id);
        let message: CatanMessage = response.json().await.unwrap();
        thread_info!(name, "long_poll returned: {:?}", message);
        if let CatanMessage::GameCreated(data) = message.clone() {
            game_id = data.game_id.clone()
        }
        thread_info!(name, "sending message: {:#?}", message);
        if let Err(e) = tx.send(message.clone()).await {
            thread_info!(name, "Failed to send message: {}", e);
        }
    }
}
