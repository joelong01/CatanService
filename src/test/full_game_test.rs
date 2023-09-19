#[cfg(test)]
pub mod test {
    #![allow(unused_imports)]
    #![allow(dead_code)]
    #![allow(unused_variables)]
    use crate::{
        games_service::shared::game_enums::CatanGames,
        middleware::request_context_mw::TestContext,
        shared::{
            proxy::ServiceProxy,
            shared_models::{PersonalInformation, UserType},
        },
        test::{
            client0::{load_game, save_game, Handler0, TEST_GAME_LOC},
            client1::Handler1,
            client2::Handler2,
            test_structs::{init_test_logger, ClientThreadHandler, HOST_URL},
        },
    };

    use std::{
        env,
        os::unix::thread,
        sync::Arc,
        time::{Duration, Instant},
    };

    use crate::{
        create_service, create_test_service, full_info,
        games_service::{
            game_container::{
                self,
                game_messages::{CatanMessage, GameHeader, Invitation},
            },
            shared::game_enums::GameState,
        },
        setup_test,
        shared::shared_models::{ClientUser, ServiceResponse, UserProfile},
        trace_thread_info,
    };
    use crate::{games_service::game_container::game_messages::ErrorData, init_env_logger};
    use actix_web::{http::header, test, HttpServer};
    use azure_core::auth;
    use futures::stream::FuturesUnordered;
    use futures::StreamExt;
    use log::{error, info, trace};
    use reqwest::{Client, StatusCode};
    use serde::{Deserialize, Serialize};
    use serde_json::json;
    use std::io;
    use tokio::{
        sync::{
            mpsc::{self, Receiver, Sender},
            Barrier, RwLock,
        },
        time::sleep,
    };
    use url::Url;
    /**
     * if you ever change the game structure, you are going to need a new copy of it. this will
     * start a game and get a new board.  be careful because if you have specific logic around
     * the layout, that will all break...you might instead just merge the old with the new, preserving
     * the tile layout.
     */
    async fn save_game_test() {
        start_server().await.unwrap();
        trace_thread_info!("test_thread", "created server");
        let client = Client::new();
        wait_for_server_to_start(&client, Duration::from_secs(10))
            .await
            .expect("Server not started");
        trace_thread_info!("test_thread", "starting test_lobby_invite_flow");

        //
        //  setup the test database
        trace_thread_info!("test_thread", "setting up service");
        let proxy = ServiceProxy::new_non_auth(
            Some(TestContext {
                use_cosmos_db: false,
            }),
            HOST_URL,
        );
        let response = proxy.setup().await;
        response.assert_success("setup should not fail");
        assert!(response.status.is_success(), "error: {:#?}", response);

        //
        //  create new users to play our game
        const CLIENT_COUNT: &'static usize = &1;
        trace_thread_info!("test_thread", "creating users");
        let test_users: Vec<ClientUser> = register_test_users(*CLIENT_COUNT).await;
        assert_eq!(test_users.len(), *CLIENT_COUNT);

        // start a game
        let returned_game = proxy
            .new_game(CatanGames::Regular, None)
            .await
            .get_game()
            .expect("Should have a RegularGame returned in the body");

        // save the game
        save_game(&returned_game);

        let test_game = load_game().expect(&format!("Test game should be in {}", TEST_GAME_LOC));
        assert_eq!(test_game, returned_game);
    }

    #[actix_rt::test]
    async fn full_game_test() {
        //
        //  first start with the setting up the service
        start_server().await.unwrap();
        trace_thread_info!("test_thread", "created server");
        let client = Client::new();
        wait_for_server_to_start(&client, Duration::from_secs(10))
            .await
            .expect("Server not started");
        trace_thread_info!("test_thread", "starting test_lobby_invite_flow");

        //
        //  setup the test database
        trace_thread_info!("test_thread", "setting up service");
        let proxy = ServiceProxy::new_non_auth(
            Some(TestContext {
                use_cosmos_db: false,
            }),
            HOST_URL,
        );
        let response = proxy.setup().await;
        response.assert_success("setup should not fail");
        assert!(response.status.is_success(), "error: {:#?}", response);

        //
        //  create new users to play our game
        const CLIENT_COUNT: &'static usize = &3;
        trace_thread_info!("test_thread", "creating users");
        let mut test_users: Vec<ClientUser> = register_test_users(*CLIENT_COUNT).await;
        assert_eq!(test_users.len(), *CLIENT_COUNT);
        //
        //  these are the handlers for clients0, clients1, and clients2
        let handlers: Vec<Box<dyn ClientThreadHandler + Send + Sync>> =
            vec![Box::new(Handler0), Box::new(Handler1), Box::new(Handler2)];

        // the handles to the client threads we are creating -- when all the client threads
        // return, this main test thread will complete and we'll see if we passed or not
        let mut handles = Vec::new();

        //
        //  create the client
        for (i, handler) in handlers.into_iter().enumerate() {
            trace_thread_info!("test_thread", "creating clients: {}", i);

            let (tx, rx) = mpsc::channel::<CatanMessage>(32);
            let username = test_users[i]
                .user_profile
                .pii
                .as_mut()
                .unwrap()
                .email
                .clone();
            trace_thread_info!(
                "test_thread",
                "starting polling thread for {}",
                username.clone()
            );
            let _ = tokio::spawn(async move {
                crate::test::polling_thread::game_poller(&username, tx).await;
            });

            let handle = tokio::spawn(async move {
                handler.run(rx).await;
            });
            handles.push(handle);
        }

        let handles: FuturesUnordered<_> = handles.into_iter().collect();
        trace_thread_info!("test_thread", "test_thread: waiting for client_threads");
        handles
            .for_each_concurrent(None, |result| async move {
                match result {
                    Ok(_) => {
                        trace_thread_info!("test_thread", "client exited ok")
                    }
                    Err(e) => {
                        trace_thread_info!("test_thread", "client exited withe erro: {:#?}", e)
                    }
                }
            })
            .await;

        trace_thread_info!("test_thread", "client_threads all finished");
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
        init_test_logger().await;
        let url = Url::parse(HOST_URL).expect("Global test URL should always parse");
        let host = url.host_str().expect("URL better have a host...");
        let port = url.port().expect("port needs to be set");
        let server = HttpServer::new(move || create_service!())
            .bind(format!("{}:{}", host, port))?
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

    async fn register_test_users(count: usize) -> Vec<ClientUser> {
        let mut test_users: Vec<ClientUser> = Vec::new();
        let proxy = ServiceProxy::new_non_auth(
            Some(TestContext {
                use_cosmos_db: false,
            }),
            HOST_URL,
        );
        let first_names = vec!["Joe", "James", "Doug"];
        let last_names = vec!["Winner", "Loser", "Longestroad"];
        let email_names = vec![
            "joe@longshotdev.com",
            "james@longshotdev.com",
            "doug@longshotdev.com",
        ];
        for i in 0..count {
            trace_thread_info!("TestThread", "creating: {}", first_names[i].clone());

            let user_profile = UserProfile {
                user_id: None,
                user_type: UserType::Connected,
                pii: Some(PersonalInformation {
                    email: email_names[i].into(),
                    phone_number: crate::middleware::service_config::SERVICE_CONFIG
                        .test_phone_number
                        .to_owned(),
                    first_name: first_names[i].into(),
                    last_name: last_names[i].into(),
                }),

                display_name: format!("{}:({})", first_names[i].clone(), i),
                picture_url: "https://example.com/photo.jpg".into(),
                foreground_color: "#000000".into(),
                background_color: "#FFFFFF".into(),
                text_color: "#000000".into(),
                games_played: Some(0),
                games_won: Some(0),
            };

            let client_user = proxy
                .register(&user_profile, "password")
                .await
                .assert_success("Register should succeed")
                .to_client_user()
                .expect("Register should have a ClientUser in the body");
            test_users.push(client_user);
        }

        test_users
    }
}
