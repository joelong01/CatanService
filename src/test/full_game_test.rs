#[cfg(test)]
pub mod test {
    #![allow(unused_imports)]
    #![allow(dead_code)]
    #![allow(unused_variables)]
    use crate::{
        games_service::shared::game_enums::CatanGames,
        shared::proxy::ServiceProxy,
        test::{
            client0::Handler0,
            client1::Handler1,
            client2::Handler2,
            polling_thread::{self, polling_thread},
            test_structs::{ClientData, ClientThreadHandler, HOST_URL, init_test_logger},
        }, wait_for_message,
    };
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
    use std::io;
    use tokio::{
        sync::{
            mpsc::{Receiver, Sender, self},
            Barrier, RwLock,
        },
        time::sleep,
    };
    use url::Url;

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
        thread_info!("test_thread", "setting up service");
        let proxy = ServiceProxy::new(true, HOST_URL);
        let response = proxy.setup().await.unwrap();
        assert!(response.status().is_success(), "error: {:#?}", response);

        //
        //  create new users to play our game
        const CLIENT_COUNT: &'static usize = &3;
        thread_info!("test_thread", "creating users");
        let test_users: Vec<ClientUser> = register_test_users(*CLIENT_COUNT).await;
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
            thread_info!("test_thread", "creating clients: {}", i);
 
            let (tx, rx) = mpsc::channel::<CatanMessage>(32);
            let username = test_users[i].user_profile.email.clone();
            thread_info!("test_thread", "starting polling thread for {}", username.clone());
            let _ = tokio::spawn(async move {
                polling_thread::polling_thread(&username, tx).await;
            });

            let handle = tokio::spawn(async move {
                handler.run(rx).await;
            });
            handles.push(handle);
        }

        let handles: FuturesUnordered<_> = handles.into_iter().collect();
        thread_info!("test_thread", "test_thread: waiting for client_threads");
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
        init_test_logger().await;
        let url = Url::parse(HOST_URL).expect("Global test URL should always parse");
        let host = url.host_str().expect("URL better have a host...");
        let port = url.port().expect("port needs to be set");
        let server = HttpServer::new(move || create_app!())
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
        let proxy = ServiceProxy::new(true, HOST_URL);
        let first_names = vec!["Joe", "James", "Doug"];
        let last_names = vec!["Winner", "Loser", "Longestroad"];
        let email_names = vec![
            "joe@longshotdev.com",
            "james@longshotdev.com",
            "doug@longshotdev.com",
        ];
        for i in 0..count {
            thread_info!("TestThread", "creating: {}", first_names[i].clone());

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
          
            let client_user = proxy.register(&user_profile, "password").await;
            test_users.push(client_user);
        }

        test_users
    }

    async fn client_thread(
        user_index: usize,
        clients: &mut Vec<ClientData>,
        mut rx: Receiver<CatanMessage>,
    ) {
        let my_info = clients[user_index].clone();
        let auth_token = my_info.auth_token.clone();
        let name = my_info.user_profile.display_name.clone();
        let proxy = ServiceProxy::new(true, HOST_URL);

        thread_info!(name, "Waiting for 500ms");
        tokio::time::sleep(Duration::from_millis(500)).await;
        thread_info!(
            name,
            "Client thread. Waiting on Start Message from poll thread"
        );
        let message = wait_for_message!(name, rx);

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
            let message = wait_for_message!(name, rx);

            game_id = match message {
                CatanMessage::GameUpdate(_) => {
                    panic!("unexpected GameUpdate");
                }
                CatanMessage::Invite(_) => {
                    panic!("wrong message received. expected GameUpdate, got Invite");
                }
                CatanMessage::InvitationResponse(_) => {
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
                CatanMessage::Started(_) => todo!(),
                CatanMessage::Ended(_) => todo!(),
                CatanMessage::PlayerAdded(_) => todo!(),
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

            for user_id in lobby {
                let my_clone = my_info.clone();
                let profile = my_clone.user_profile;

                let invite_message = "Join my game!".to_string();
                let invitation = Invitation {
                    originator_id: my_clone.client_id.clone(),
                    recipient_id: user_id.clone(),
                    originator_name: profile.display_name.clone(),
                    message: invite_message.clone(),
                    picture_url: profile.picture_url.clone(),
                    game_id: game_id.to_owned(),
                };
                thread_info!(name, "Sending GameInvite");
                let response = proxy.send_invite(&invitation, &auth_token).await.unwrap();
                assert!(
                    response.status().is_success(),
                    "error loggin in user: {}, err: {:#?}",
                    my_info.user_profile.display_name,
                    response
                );
            }
        } else {
            // end first user creates game an invites everybody
            let message = wait_for_message!(name, rx);
            let invitation = match message {
                CatanMessage::Invite(invitation) => {
                    //    assert_eq!(invitation.game_id, game_id);
                    thread_info!(
                        name,
                        "recieved invitation for game_id: {} from: {}",
                        invitation.game_id,
                        invitation.originator_name.clone()
                    );
                    invitation // return the invitation if the variant is Invite
                }
                CatanMessage::GameUpdate(_) => {
                    panic!("wrong message received"); // you can use panic! or assert! as per your requirement
                }
                CatanMessage::InvitationResponse(_) => {
                    panic!("wrong message received. expected GameUpdate, got InviteAccepted");
                }
                CatanMessage::Error(_) => {
                    panic!("error message received");
                }
                CatanMessage::GameCreated(_) => {
                    panic!("got GameCreated msg");
                }
                CatanMessage::Started(_) => todo!(),
                CatanMessage::Ended(_) => todo!(),
                CatanMessage::PlayerAdded(_) => todo!(),
            };

            assert_eq!(invitation.game_id, game_id);
        }

        let message = wait_for_message!(name, rx);
        // next we start the game
    }

    // async fn game_thread_message_loop(
    //     name: &str,
    //     auth_token: &str,
    //     client_id: &str,
    //     mut rx: Receiver<CatanMessage>,
    // ) {
    //     let proxy = ServiceProxy::new(true, HOST_URL);
    //     loop {
    //         let message = wait_for_message!(name, rx);
    //         match message {
    //             CatanMessage::GameUpdate(_) => todo!(),
    //             CatanMessage::Invite(invitation) => {
    //                 thread_info!(
    //                     "name",
    //                     "Invitation received from {}",
    //                     invitation.originator_name
    //                 );
    //                 // respond to invite
    //                 let response = proxy.invitation_response(&invitation, auth_token).await.unwrap();
    //                 assert!(response.status().is_success(), "error: {:#?}", response);
    //             }
    //             CatanMessage::Error(e) => {
    //                 panic!("Error message received: {:#?}", e);
    //             }
    //             CatanMessage::InvitationResponse(_) => todo!(),
    //             CatanMessage::GameCreated(_) => todo!(),
    //             CatanMessage::Started(_) => todo!(),
    //             CatanMessage::Ended(_) => {
    //                 break;
    //             }
    //             CatanMessage::PlayerAdded(_) => todo!(),
    //         }
    //     }
    // }
}
