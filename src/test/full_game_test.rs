#[cfg(test)]
mod test {
    #![allow(unused_imports)]
    #![allow(dead_code)]
    use crate::{games_service::shared::game_enums::CatanGames, shared::proxy::ServiceProxy};
    use std::{
        os::unix::thread,
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
        thread_info,
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
    use serial_test::serial;
    use std::io;
    use tokio::{
        sync::{
            mpsc::{Receiver, Sender},
            Barrier, RwLock,
        },
        time::sleep,
    };

    const HOST_NAME: &str = "http://localhost:8082";
    const HOST_URL: &str = "localhost:8082";

    #[derive(Clone, Serialize, Deserialize, Eq, PartialEq)]
    struct UserInfo {
        auth_token: String,
        user_profile: UserProfile,
        client_id: String,
    }

    #[derive(Clone)]
    struct ClientData {
        pub auth_token: String,
        pub user_profile: UserProfile,
        pub client_id: String,
    }

    impl ClientData {
        fn new(client_id: &str, user_profile: &UserProfile, auth_token: &str) -> Self {
            Self {
                auth_token: auth_token.to_string(),
                user_profile: user_profile.clone(),
                client_id: client_id.to_string(),
            }
        }
    }

    #[actix_rt::test]
    async fn full_game_test() {
        //
        //  first start with the setting up the service
        start_server().await.unwrap();
        thread_info!("test_thread", "created server");
        let client = Client::new();
        wait_for_server_to_start(&client, Duration::from_secs(10))
            .await
            .expect("Server not started");
        thread_info!("test_thread", "starting test_lobby_invite_flow");

        //
        //  setup the test database

        let proxy = ServiceProxy::new(true, HOST_NAME);
        let response = proxy.setup().await.unwrap();
        assert!(response.status().is_success(), "error: {:#?}", response);

        //
        //  create new users to play our game
        const CLIENT_COUNT: &'static usize = &3;
        let simulated_clients: Vec<ClientData> = create_users(*CLIENT_COUNT).await;

        // some synchronization structs
        let mut handles = Vec::new();
        let init_barrier = Arc::new(Barrier::new(*CLIENT_COUNT + 1));
        //
        //  create the client
        for i in 0..*CLIENT_COUNT {
            let barrier = Arc::clone(&init_barrier);
            let (tx, rx) = tokio::sync::mpsc::channel::<CatanMessage>(32); // Buffer size of 32
            let mut cloned_clients = simulated_clients.clone();
            let handle = tokio::spawn(async move {
                barrier.wait().await;
                client_thread(i, &mut cloned_clients, tx, rx).await;
            });
            handles.push(handle);
        }
        thread_info!("test_thread", "test_thread: waiting for init_barrier");
        init_barrier.wait().await;
        thread_info!("test_thread", "test_thread: past init_barrier");

        thread_info!("test_thread", "test_thread: waiting for client_threads");
        let handles: FuturesUnordered<_> = handles.into_iter().collect();

        handles
            .for_each_concurrent(None, |result| async move {
                match result {
                    Ok(_) => {
                        thread_info!("test_thread", "client exited ok")
                    }
                    Err(e) => {
                        thread_info!("test_thread", "client exited withe erro: {:#?}", e)
                    }
                }
            })
            .await;

        thread_info!("test_thread", "client_threads all finished");
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
        let server = HttpServer::new(move || create_app!()).bind(HOST_URL)?.run();

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
        auth_token: &str,
        barrier: &Barrier,
        tx: tokio::sync::mpsc::Sender<CatanMessage>,
    ) {
        let proxy = ServiceProxy::new(true, HOST_NAME);
        // Create the client inside the spawned task

        thread_info!(name, "polling thread started, waiting on barrier");
        barrier.wait().await;
        thread_info!(name, "Barrier Clear");

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
                    thread_info!(name, "pulled invitation: {:#?}", invite_data);
                }
                CatanMessage::InviteAccepted(accept_message) => {
                    game_id = accept_message.game_id.to_owned(); // need the game_id to switch to different queue
                    thread_info!(
                        name,
                        "pulled InviteAccepted.  game_id: {:#?}",
                        game_id.clone()
                    );
                }
                CatanMessage::Error(error_data) => {
                    thread_info!(name, "pulled an error!: {:#?}", error_data);
                    assert!(false, "error returned:  {:#?}", error_data);
                    break;
                }
                CatanMessage::GameCreated(msg) => {
                    thread_info!(name, "pulled an GameCreated: {:#?}", msg);
                    game_id = msg.game_id.to_owned(); // need the game_id to switch to different queue
                }
            }

            thread_info!(name, "sending message: {:#?}", message);
            if let Err(e) = tx.send(message.clone()).await {
                thread_info!(name, "Failed to send message: {}", e);
            }
        }
    }

    async fn create_users(count: usize) -> Vec<ClientData> {
        let mut simulated_clients: Vec<ClientData> = Vec::new();
        let proxy = ServiceProxy::new(true, HOST_NAME);
        let first_names = vec!["Joe", "James", "Doug"];
        let last_names = vec!["Winner", "Loser", "Longestroad"];
        let email_names = vec![
            "joe@longshotdev.com",
            "dodgy@longshotdev.com",
            "doug@longshotdev.com",
        ];
        for i in 0..count {
            thread_info!("TestThread", "creating: {}", first_names[i].clone());
            let password = "password";
            let user_profile = UserProfile {
                email: email_names[i].into(),
                first_name: first_names[i].into(),
                last_name: last_names[i].into(),
                display_name: format!("{}:({})", first_names[i].clone(), i),
                picture_url: "https://example.com/photo.jpg".into(),
                foreground_color: "#000000".into(),
                background_color: "#FFFFFF".into(),
                text_color: "#000000".into(),
                games_played: Some(0),
                games_won: Some(0),
            };

            let response = proxy.register(&user_profile, password).await.unwrap();
            assert!(
                response.status().is_success(),
                "error registering user: {}, err: {:#?}",
                user_profile.display_name,
                response
            );

            let response = proxy.login(email_names[i], password).await;
            let response = match response {
                Ok(r) => r,
                Err(e) => {
                    panic!(
                        "error loggin in user: {}, err: {:#?}",
                        user_profile.display_name, e
                    )
                }
            };
            assert!(
                response.status().is_success(),
                "error loggin in user: {}, err: {:#?}",
                user_profile.display_name,
                response
            );

            let body = response.text().await.unwrap();
            let service_response: ServiceResponse = serde_json::from_str(&body).unwrap();

            // Extract auth token from response
            let auth_token = service_response.body;

            // get the profile
            let response = proxy.get_profile(&auth_token).await.unwrap();
            assert!(
                response.status().is_success(),
                "error loggin in user: {}, err: {:#?}",
                user_profile.display_name,
                response
            );

            let client_user: ClientUser = response.json().await.unwrap();

            assert!(
                client_user.user_profile.is_equal_byval(&user_profile),
                "profile returned by service different than the one sent in"
            );
            let client = ClientData::new(&client_user.id, &client_user.user_profile, &auth_token);
            simulated_clients.push(client);
        }

        simulated_clients
    }

    async fn client_thread(
        user_index: usize,
        clients: &mut Vec<ClientData>,
        tx: Sender<CatanMessage>,
        mut rx: Receiver<CatanMessage>,
    ) {
        let my_info = clients[user_index].clone();
        let my_info_for_closure = my_info.clone();
        let auth_token = my_info.auth_token.clone();
        let local_barrier = Arc::new(Barrier::new(2));
        let local_barrier_clone = local_barrier.clone();
        let name = my_info.user_profile.display_name.clone();
        let client = reqwest::Client::new();
        let proxy = ServiceProxy::new(true, HOST_NAME);
        // spawn the long poller
        let _ = tokio::spawn(async move {
            local_barrier_clone.wait().await;
            polling_thread(
                &my_info_for_closure.user_profile.display_name.clone(),
                &my_info_for_closure.auth_token,
                &local_barrier_clone,
                tx,
            )
            .await;
        });
        thread_info!(name, "Client thread. Waiting on Barrier");

        local_barrier.wait().await;
        thread_info!(name, "Client thread. Cleared on Barrier");
        let mut game_id = "".to_string();

        //
        //  in the browser app, the browser worker is up and running before the UI, so you don't
        //  need to worry the issue of the main thread running before the polling threads. here 
        //  we do -- so we just go to sleep for a bit.
        thread_info!(name, "Sleeping for 1 second...");
        sleep(Duration::from_secs(1)).await;
        thread_info!(name, "Game Thread Woke up!");

        //
        // create the game if the index is 0
        if user_index == 0 {
            full_info!("creating new game with token {}", auth_token.clone());
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
            thread_info!(name, "New Game created.  Waiting for message.");
            let message = rx
                .recv()
                .await
                .unwrap_or_else(|| panic!("failed to receive message"));

            game_id = match message {
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
                    thread_info!(
                        name,
                        "Received GameCreated Message.  game_id: {}",
                        msg.game_id.clone()
                    );
                    msg.game_id
                }
            };
            assert!(game_id != "");

            //
            // get the lobby
            thread_info!(name, "Getting Lobby.");
            let response = proxy.get_lobby(&auth_token).await.unwrap();
            assert!(
                response.status().is_success(),
                "error loggin in user: {}, err: {:#?}",
                my_info.user_profile.display_name,
                response
            );

            let lobby: Vec<String> = response.json().await.unwrap();
            thread_info!(name, "get_lobby returned: {:#?}", lobby);
            assert!(lobby.len() == 2); // there should be two people waiting in the lobby

            for i in 1..clients.len() - 2 {
                let my_clone = my_info.clone();
                let profile = my_clone.user_profile;

                let invite_message = "Join my game!".to_string();
                let invitation_to_user1 = Invitation {
                    from_id: my_clone.client_id.clone(),
                    to_id: clients[i].client_id.clone(),
                    from_name: profile.display_name.clone(),
                    message: invite_message.clone(),
                    picture_url: profile.picture_url.clone(),
                    game_id: game_id.to_owned(),
                };

                let url = "http://127.0.0.1:8082/auth/api/v1/lobby/invite"; // Adjust the URL as needed
                thread_info!(name, "Sending GameInvite");
                let response = client
                    .post(url)
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(GameHeaders::IS_TEST, "true")
                    .header("Authorization", my_clone.auth_token.clone())
                    .json(&invitation_to_user1)
                    .send()
                    .await
                    .unwrap();

                assert_eq!(response.status(), 200);
            }
        } else {
            // end first user creates game an invites everybody
            thread_info!(name, "waiting for invite message");
            let message = rx
                .recv()
                .await
                .unwrap_or_else(|| panic!("failed to receive message"));
            thread_info!(name, "recieved message");
            let invitation = match message {
                CatanMessage::Invite(invitation) => {
                    assert_eq!(invitation.game_id, game_id);
                    thread_info!(
                        name,
                        "recieved invitation for game_id: {} from: {}",
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

        // next we start the game
    }
}
