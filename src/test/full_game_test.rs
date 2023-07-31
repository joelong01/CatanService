mod test {
    #![allow(unused_imports)]
    #![allow(dead_code)]
    use std::sync::Arc;

    use crate::{
        create_app, create_test_service, setup_test,
        shared::models::{ClientUser, ServiceResponse, UserProfile}, games_service::{game_container::game_messages::CatanMessage, shared::game_enums::GameState},
    };
    use actix_web::{http::header, test, HttpServer};
    use log::{info, trace};
    use reqwest::Client;
    use serde::{Deserialize, Serialize};
    use serde_json::json;
    use std::io;
    use tokio::sync::{Barrier, RwLock};

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
        HttpServer::new(move || create_app!())
            .bind("127.0.0.1:8082")?
            .run()
            .await
    }
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

        let client = Client::new();

        info!("starting test_lobby_invite_flow");
        #[derive(Clone, Serialize, Deserialize, Eq, PartialEq)]
        struct UserInfo {
            auth_token: String,
            user_profile: UserProfile,
            client_id: String,
        }
        let mut user_info_list: Vec<UserInfo> = Vec::new();
        for i in 0..3 {
            let email = format!("test_lobby_invite_flow_{}@example.com", i);
            let password = "password".to_string();
            let mut client_id: String = "".to_string();
            let user_profile = UserProfile {
                email: email.clone(),
                first_name: "Test".into(),
                last_name: "User".into(),
                display_name: format!("TestUser{}", i),
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
                .header("x-is_test", "true")
                .header("X-Password", password.clone())
                .json(&user_profile)
                .send()
                .await
                .unwrap();

            let status = response.status();
            let body = response.text().await.unwrap();
            if status != 200 {
                trace!("{} already registered", email);
                assert_eq!(status, 409);
                let resp: ServiceResponse = serde_json::from_str(&body)
                    .expect("failed to deserialize into ServiceResponse");
                assert_eq!(resp.status, 409);
                assert_eq!(resp.body, "");
            } else {
                // Deserialize the body into a ClientUser object
                let client_user: ClientUser =
                    serde_json::from_str(&body).expect("Failed to deserialize response body");
                client_id = client_user.id;
            }

            let login_url = "http://127.0.0.1:8082/api/v1/users/login"; // Adjust the URL as needed
            let login_payload = json!({
                "username": email.to_string(),
                "password": password.clone()
            });

            let response = client // Reusing the existing client
                .post(login_url)
                .header("x-is_test", "true")
                .json(&login_payload)
                .send()
                .await
                .unwrap();

            assert_eq!(response.status(), 200);

            let service_response: ServiceResponse = response.json().await.unwrap();

            // Extract auth token from response
            let auth_token = service_response.body;

            let profile_url = "http://127.0.0.1:8082/auth/api/v1/profile"; // Adjust the URL as needed

            let response = client // Reusing the existing client
                .get(profile_url)
                .header(header::CONTENT_TYPE, "application/json")
                .header("x-is_test", "true")
                .header("Authorization", auth_token.clone())
                .send()
                .await
                .unwrap();

            assert_eq!(response.status(), 200);

            let profile_from_service: UserProfile = response.json().await.unwrap();

            assert!(
                profile_from_service.is_equal_byval(&user_profile),
                "profile returned by service different than the one sent in"
            );

            let user_info = UserInfo {
                auth_token: auth_token.clone(),
                user_profile: profile_from_service.clone(),
                client_id: client_id.clone(),
            };
            user_info_list.push(user_info);
        }
        for info in user_info_list.clone() {
            info!(
                "email {}, token-len: {}, id: {}",
                info.user_profile.email,
                info.auth_token.len(),
                info.client_id
            );
        }

        // we now have 3 users that we can use to play a game.
        // first step is 3 threads to wait on longpoll

        let barrier = Arc::new(Barrier::new(4));

        for info in user_info_list {
            let auth_token_clone = info.auth_token.clone();
            let barrier_clone = barrier.clone();
          //  let invite_message = "Join my game!".to_string();
            let longpoll_url = "http://127.0.0.1:8082/auth/api/v1/longpoll"; // Adjust the URL as needed

            let _ = tokio::spawn(async move {
                // Create the client inside the spawned task
                let client = reqwest::Client::new();

                info!(
                    "{} thread started.  calling barrier_clone().wait().await",
                    info.client_id
                );
                barrier_clone.wait().await; // barrier at 3
                let mut game_id = "".to_string();

                loop {
                    let response = client
                        .post(longpoll_url)
                        .header(header::CONTENT_TYPE, "application/json")
                        .header("x-is_test", "true")
                        .header("x-game_id", game_id.to_owned())
                        .header("Authorization", auth_token_clone.clone())
                        .send()
                        .await
                        .unwrap();

                    assert_eq!(response.status(), 200);

                    let message:CatanMessage = response.json().await.unwrap();

                    match message {
                        CatanMessage::GameUpdate(regular_game) => {
                            info!("players: {:#?}", regular_game.players);
                            if regular_game.state_data.state() == GameState::GameOver {
                                break;
                            }
                        },
                        CatanMessage::Invite(invite_data) => {
                            game_id = invite_data.game_id.to_owned();
                            assert_eq!(invite_data.from_name, "TestUser1");
                        },
                        CatanMessage::Error(error_data) => {
                           assert!(false, "error returned:  {:#?}", error_data);
                           break;
                        },
                    }

                    // Optionally, you may want to add a delay or condition to break the loop if needed.
                }
            });
        }
    }
}
