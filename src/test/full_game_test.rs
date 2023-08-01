#[cfg(test)]
mod test {
    #![allow(unused_imports)]
    #![allow(dead_code)]
    use std::{
        sync::Arc,
        time::{Duration, Instant},
    };

    use crate::{
        create_app, create_test_service, full_info,
        games_service::{
            game_container::{
                self,
                game_messages::{CatanMessage, GameHeaders, Invitation},
            },
            shared::game_enums::GameState,
        },
        setup_test,
        shared::models::{ClientUser, ServiceResponse, UserProfile},
    };
    use crate::{games_service::game_container::game_messages::ErrorData, init_env_logger};
    use actix_web::{http::header, test, HttpServer};
    use log::{error, info, trace};
    use reqwest::{Client, StatusCode};
    use serde::{Deserialize, Serialize};
    use serde_json::json;
    use serial_test::serial;
    use std::io;
    use tokio::sync::{Barrier, RwLock};

    /**
     *
     * I have to create a "real" web server to send the requests to instead of starting a test service because
     * I need to spawn and call from different threads and passing the app to the threads proved to be difficult/
     * impossible. this starts a service (the same as it would in main) and then calls it like a rust client would.
     * the only difference is I don't bother using HTTPS
     *
     * 1. create the app
     * 2. add 3 users
     * 3. user 1 and 2 wait for invites
     * 4. user 3 creates the game
     * 5. user 3 sends an invite to user 2.
     * 6. User 2 accepts invite and then waits for game changes
     * 7. user 3 sends invite to user 1
     * 8. user 1 accepts invite
     * 9. user 3 starts game.
     *
     * TODO:  test the reject path
     */
    #[actix_rt::test]
    async fn full_game_test() {
        start_server().await.unwrap();
        full_info!("created server");
        let client = Client::new();
        wait_for_server_to_start(&client, Duration::from_secs(10))
            .await
            .expect("Server not started");
        full_info!("starting test_lobby_invite_flow");
        #[derive(Clone, Serialize, Deserialize, Eq, PartialEq)]
        struct UserInfo {
            auth_token: String,
            user_profile: UserProfile,
            client_id: String,
        }
        const CLIENT_COUNT: &'static usize = &3;
        let mut user_info_list: Vec<UserInfo> = Vec::new();
        for i in 0..*CLIENT_COUNT {
            let email = format!("test_{}@example.com", i);
            let password = "password".to_string();
            let user_profile = UserProfile {
                email: email.clone(),
                first_name: "Test".into(),
                last_name: "User".into(),
                display_name: format!("TestUser:{}", i),
                picture_url: "https://example.com/photo.jpg".into(),
                foreground_color: "#000000".into(),
                background_color: "#FFFFFF".into(),
                text_color: "#000000".into(),
                games_played: Some(0),
                games_won: Some(0),
            };
            
            
            let register_url = "http://127.0.0.1:8082/api/v1/users/register";
            let response = client
                .post(register_url)
                .header(GameHeaders::IS_TEST, "true")
                .header(GameHeaders::PASSWORD, password.clone())
                .json(&user_profile)
                .send()
                .await;

            match response {
                Ok(response_value) => {
                    if response_value.status().is_success() {
                        trace!("created user {:#?}", response_value);
                    } else {
                        // Status indicates failure, attempt to deserialize error body
                        let body = response_value.text().await.unwrap_or_default();
                        match serde_json::from_str::<ServiceResponse>(&body) {
                            Ok(service_response) => {
                                trace!(
                                    "{} already registered. error: {:#?}",
                                    email,
                                    service_response
                                );
                            }
                            Err(err) => {
                                trace!("Failed to deserialize error response: {}", err);
                            }
                        };
                    }
                }
                Err(error) => {
                    trace!("Request failed: {:#?}", error);
                }
            };

            let login_url = "http://127.0.0.1:8082/api/v1/users/login"; // Adjust the URL as needed
            #[derive(Serialize, Deserialize)]
            struct LoginBody {
                username: String,
                password: String,
            }
            let login_payload = LoginBody {
                username: email.to_string(),
                password: password.clone(),
            };

            let response = client // Reusing the existing client
                .post(login_url)
                .header(GameHeaders::IS_TEST, "true")
                .json(&login_payload)
                .send()
                .await;
            let response = match response {
                Ok(response_value) => response_value,
                Err(error) => {
                    // Handle the error accordingly
                    panic!("Request failed: {:?}", error);
                }
            };

            let status = response.status();
            full_info!("Response status: {}", status);
            let body = response.text().await.unwrap();
            if !status.is_success() {
                if status.is_client_error() {
                    panic!(
                        "error logging in: {} for {} and {}",
                        status, login_payload.username, login_payload.password
                    );
                } else {
                    // Handle server error status codes (5xx)
                    panic!("got a 5xx call trying to login!!")
                }
            }

            let service_response: ServiceResponse = serde_json::from_str(&body).unwrap();

            // Extract auth token from response
            let auth_token = service_response.body;

            let profile_url = "http://127.0.0.1:8082/auth/api/v1/profile"; // Adjust the URL as needed
            full_info!("logging in {}", user_profile.email);
            let response = client // Reusing the existing client
                .get(profile_url)
                .header(header::CONTENT_TYPE, "application/json")
                .header(GameHeaders::IS_TEST, "true")
                .header("Authorization", auth_token.clone())
                .send()
                .await
                .unwrap();

            assert_eq!(response.status(), 200);

            let client_user: ClientUser = response.json().await.unwrap();

            assert!(
                client_user.user_profile.is_equal_byval(&user_profile),
                "profile returned by service different than the one sent in"
            );

            let user_info = UserInfo {
                auth_token: auth_token.clone(),
                user_profile: client_user.user_profile.clone(),
                client_id: client_user.id.clone(),
            };
            user_info_list.push(user_info);
        }
        for info in user_info_list.clone() {
            full_info!(
                "email {}, token-len: {}, id: {}",
                info.user_profile.email,
                info.auth_token.len(),
                info.client_id
            );
        }

        struct TestClientMessage {
            client_name: String,
            message: CatanMessage,
        }

        // we now have 3 users that we can use to play a game.
        // first step is 3 threads to wait on longpoll
        let (tx, mut rx) = tokio::sync::mpsc::channel::<CatanMessage>(32); // Buffer size of 32

        let barrier = Arc::new(Barrier::new(*CLIENT_COUNT));

        for info in user_info_list.clone() {
            let auth_token_clone = info.auth_token.clone();
            let barrier_clone = barrier.clone();
            let tx_clone = tx.clone(); // Clone the sender for each task
            let _ = tokio::spawn(async move {
                polling_thread(
                    &info.user_profile.email,
                    auth_token_clone,
                    &barrier_clone,
                    tx_clone,
                )
                .await; // Await here, inside the spawned task
            });
        }

        // 3 clients are created - they are the "listeners" that run in the web worker in the react app
        // the main thread needs to wait for the threads to spawn

        full_info!("Main thread: Waiting on Barrier");
        barrier.wait().await; // Wait for the main task
        full_info!("Main thread: Cleared on Barrier");

        //
        // create the game
        let url = "http://127.0.0.1:8082/auth/api/v1/games/Regular"; // Adjust the URL as needed
        full_info!(
            "creating new game with token {}",
            user_info_list[0].auth_token.clone()
        );
        let response = client
            .post(url)
            .header(header::CONTENT_TYPE, "application/json")
            .header(GameHeaders::IS_TEST, "true")
            .header("Authorization", user_info_list[0].auth_token.clone())
            .send()
            .await
            .unwrap();

        assert_eq!(response.status(), 200);
        full_info!("Main Thread: New Game created.  Waiting for message.");
        let message = rx
            .recv()
            .await
            .unwrap_or_else(|| panic!("failed to receive message"));

        let game_id = match message {
            CatanMessage::GameUpdate(_) => {
                panic!("unexpected GameUpdate");
            }
            CatanMessage::Invite(_) => {
                panic!("wrong message received. expected GameUpdate, got Invite");
            }
            CatanMessage::InviteAccepted(_) => {
                panic!("wrong message received. expected GameUpdate, got InviteAccepted");
            }
            CatanMessage::Error(_) => {
                panic!("error message received");
            }
            CatanMessage::GameCreated(msg) => {
                full_info!(
                    "Received GameCreated Message.  game_id: {}",
                    msg.game_id.clone()
                );
                msg.game_id
            }
        };
        assert!(game_id != "");

        // send an invite
        let invite_message = "Join my game!".to_string();
        let invitation_to_user1 = Invitation {
            from_id: user_info_list[0].client_id.clone(),
            to_id: user_info_list[1].client_id.clone(),
            from_name: user_info_list[0].user_profile.display_name.clone(),
            message: invite_message.clone(),
            picture_url: user_info_list[0].user_profile.picture_url.clone(),
            game_id: game_id.to_owned(),
        };

        let url = "http://127.0.0.1:8082/auth/api/v1/lobby/invite"; // Adjust the URL as needed
        full_info!("MainThread:: Sending GameInvite");
        let response = client // Reusing the existing client
            .post(url)
            .header(header::CONTENT_TYPE, "application/json")
            .header(GameHeaders::IS_TEST, "true")
            .header("Authorization", user_info_list[0].auth_token.clone())
            .json(&invitation_to_user1)
            .send()
            .await
            .unwrap();

        assert_eq!(response.status(), 200);
        full_info!("MainThread:: waiting for message");
        let message = rx
            .recv()
            .await
            .unwrap_or_else(|| panic!("failed to receive message"));
        full_info!("MainThread:: recieved message");
        let invitation = match message {
            CatanMessage::Invite(invitation) => {
                assert_eq!(invitation.game_id, game_id);
                full_info!(
                    "MainThread:: recieved invitation for game_id: {} from: {}",
                    invitation.game_id,
                    invitation.from_name.clone()
                );
                invitation // return the invitation if the variant is Invite
            }
            CatanMessage::GameUpdate(_) => {
                panic!("wrong message received"); // you can use panic! or assert! as per your requirement
            }
            CatanMessage::InviteAccepted(_) => {
                panic!("wrong message received. expected GameUpdate, got InviteAccepted");
            }
            CatanMessage::Error(_) => {
                panic!("error message received");
            }
            CatanMessage::GameCreated(_) => {
                panic!("got GameCreated msg");
            }
        };

        assert_eq!(invitation.game_id, game_id);
    }

    #[allow(unused_macros)]
    macro_rules! call_service {
        ($app:expr, $req:expr) => {
            {
                let app_read_guard = $app.read().await; // Acquire a read lock
                let response = test::call_service(&mut *app_read_guard, $req).await; // Use the locked app
                response
            }
        };
    }
    async fn start_server() -> io::Result<()> {
        init_env_logger().await;
        let server = HttpServer::new(move || create_app!())
            .bind("127.0.0.1:8082")?
            .run();

        // Spawn the server's future on a new task
        tokio::spawn(async move {
            if let Err(e) = server.await {
                eprintln!("Server error: {}", e);
            }
        });

        Ok(())
    }

    async fn wait_for_server_to_start(
        client: &Client,
        timeout: Duration,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let start_time = Instant::now();
        let mut count = 0;
        loop {
            count = count + 1;
            let response = client
                .get("http://127.0.0.1:8082/api/v1/version")
                .send()
                .await;
            if let Ok(resp) = response {
                if resp.status().is_success() {
                    full_info!("count {}", count);
                    return Ok(());
                }
            }

            if start_time.elapsed() > timeout {
                return Err("Server startup timed out".into());
            }

            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    async fn polling_thread(
        name: &str,
        auth_token: String,
        barrier: &Barrier,
        tx: tokio::sync::mpsc::Sender<CatanMessage>,
    ) {
        let longpoll_url = "http://127.0.0.1:8082/auth/api/v1/longpoll"; // Adjust the URL as needed

        // Create the client inside the spawned task
        let client = reqwest::Client::new();

        full_info!("thread started. for client {}", name);
        full_info!("Barrier Wait: {}", name);
        barrier.wait().await; // barrier at 3
        full_info!("Barrier Clear: {}", name);

        let mut game_id = "".to_string();
        loop {
            full_info!("Begin poll. for client {}.  GameId: {}", name, game_id);

            let response = client
                .get(longpoll_url)
                .header(header::CONTENT_TYPE, "application/json")
                .header(GameHeaders::IS_TEST, "true")
                .header(GameHeaders::GAME_ID, game_id.to_owned())
                .header("Authorization", auth_token.clone())
                .send()
                .await;

            let response = match response {
                Ok(response_value) => response_value,
                Err(error) => {
                    // Handle the error accordingly
                    panic!("Request failed: {:?}", error);
                }
            };
            full_info!("poll returned. for client {}.  GameId: {}", name, game_id);
            let message: CatanMessage = response.json().await.unwrap();

            
            let message_clone = message.clone();
            match message_clone {
                CatanMessage::GameUpdate(regular_game) => {
                    let game = regular_game.clone();
                    full_info!("players: {:#?}", game.players);
                    if game.state_data.state() == GameState::GameOver {
                        break;
                    }
                }
                CatanMessage::Invite(invite_data) => {
                   full_info!("{} pulled invitation: {:#?}", name, invite_data);
                }
                CatanMessage::InviteAccepted(accept_message) => {
                    game_id = accept_message.game_id.to_owned(); // need the game_id to switch to different queue
                    full_info!("{} pulled InviteAccepted.  game_id: {:#?}", name, game_id.clone());
                }
                CatanMessage::Error(error_data) => {
                    full_info!("{} pulled an error!: {:#?}", name, error_data);
                    assert!(false, "error returned:  {:#?}", error_data);
                    break;
                }
                CatanMessage::GameCreated(msg) => {
                    full_info!("{} pulled GameCreated: {:#?}", name, msg);
                    game_id = msg.game_id.to_owned(); // need the game_id to switch to different queue
                }
            }

            full_info!("{} sending message: {:#?}", name, message);
            if let Err(e) = tx.send(message.clone()).await {
                error!("Failed to send message: {}", e);
            }
        }
    }
}
