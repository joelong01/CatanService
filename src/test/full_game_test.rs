mod test {
    #![allow(unused_imports)]
    use std::sync::Arc;

    use crate::{
        create_test_service, setup_test,
        shared::models::{ClientUser, ServiceResponse, UserProfile}, create_app,
    };
    use actix_web::{http::header, test};
    use log::{info, trace};
    use serde_json::json;
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
        let app = create_test_service!();
        let locked_app = Arc::new(RwLock::new(app));

       let app_clone = locked_app.read().await;
        setup_test!(&*app_clone);


        info!("starting test_lobby_invite_flow");
     
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

            let req = test::TestRequest::post()
                .uri("/api/v1/users/register")
                .append_header((header::CONTENT_TYPE, "application/json"))
                .append_header(("x-is_test", "true"))
                .append_header(("X-Password".to_owned(), password.clone()))
                .set_json(&user_profile)
                .to_request();

            let response = call_service!(&mut locked_app, req);
            let status = response.status();
            let body = test::read_body(response).await;
            if status != 200 {
                trace!("{} already registered", email);
                assert_eq!(status, 409);
                let resp: ServiceResponse = serde_json::from_slice(&body)
                    .expect("failed to deserialize into ServiceResponse");
                assert_eq!(resp.status, 409);
                assert_eq!(resp.body, "");
            } else {
                // Deserialize the body into a ClientUser object
                let client_user: ClientUser =
                    serde_json::from_slice(&body).expect("Failed to deserialize response body");
                client_id = client_user.id;
            }

            // 2. Login the user
            let login_payload = json!({
                "username": email.to_string(),
                "password": password.clone()
            });
            let req = test::TestRequest::post()
                .uri("/api/v1/users/login")
                .append_header(("x-is_test", "true"))
                .set_json(&login_payload)
                .to_request();

            let resp = call_service!(&mut locked_app, req);
            assert_eq!(resp.status(), 200);

            let body = test::read_body(resp).await;
            let service_response: ServiceResponse =
                serde_json::from_slice(&body).expect("failed to deserialize into ServiceResponse");

            // Extract auth token from response
            let auth_token = service_response.body;

            // 4. Get profile
            let req = test::TestRequest::get()
                .uri("/auth/api/v1/profile")
                .append_header((header::CONTENT_TYPE, "application/json"))
                .append_header(("x-is_test", "true"))
                .append_header(("Authorization", auth_token.clone()))
                .to_request();

            let resp = call_service!(&mut locked_app, req);
            assert_eq!(resp.status(), 200);
            let body = test::read_body(resp).await;
            let profile_from_service: UserProfile =
                serde_json::from_slice(&body).expect("error deserializing profile");

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
        for info in user_info_list {
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
            let message = "Join my game!".to_string();
            let app_clone = locked_app.clone(); // note this clones the arc, not the actuall app
            let cloned_app = app.clone();
            let user1_wait = tokio::spawn(async move {
                info!(
                    "{} thread started.  calling barrier_clone().wait().await",
                    info.client_id
                );
                barrier_clone.wait().await; // barrier at 3
                let game_id = "";

                loop {
                    let req = test::TestRequest::post()
                        .uri("/auth/api/v1/longpoll")
                        .append_header((header::CONTENT_TYPE, "application/json"))
                        .append_header(("x-is_test", "true"))
                        .append_header(("x-game_id", game_id))
                        .append_header(("Authorization", auth_token_clone.clone()))
                        .to_request();

                    let app_read_guard = app_clone.read().await; // Acquire a read lock
                    let resp = test::call_service(&mut *app_read_guard, req).await;
                    
                }
            });
        }
    }
}
