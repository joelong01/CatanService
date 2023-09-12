#![allow(dead_code)]

#[cfg(test)]
pub mod test {

    use crate::middleware::request_context_mw::{RequestContext, TestContext, SERVICE_CONFIG};
    use crate::shared::shared_models::{
        ClientUser, GameError, ResponseType, ServiceResponse, UserProfile,
    };
    use crate::user_service::users::{login, register};
    use std::io::prelude::*;
    use std::{env, fs::File};

    use crate::games_service::game_container::game_messages::GameHeader;
    use crate::{create_test_service, init_env_logger};
    use actix_web::http::header;
    use actix_web::test;
    use reqwest::StatusCode;

    #[tokio::test]
    async fn test_get_auth_token() {
        crate::init_env_logger(log::LevelFilter::Info, log::LevelFilter::Error).await;
        let _admin_token = TestHelpers::admin_login().await;
        let test_users = TestHelpers::load_test_users_from_config();
        log::trace!("{}", serde_json::to_string(&test_users).unwrap());
        let _test_context = TestContext::new(true);

        print!("ok");
    }
    #[tokio::test]
    async fn test_service_response_serialization() {
        init_env_logger(log::LevelFilter::Trace, log::LevelFilter::Error).await;
        let sr = ServiceResponse::new(
            "already exists",
            StatusCode::ACCEPTED,
            ResponseType::NoData,
            GameError::NoError(String::default()),
        );

        let json = serde_json::to_string(&sr).unwrap();
        log::info!("to_http_response: {}", json);
        match serde_json::from_str::<ServiceResponse>(&json) {
            Ok(_) => {
                log::trace!("round trip succeeded");
            }
            Err(e) => {
                panic!("failed to roundtrip ServiceResponse: {:#?}", e);
            }
        };
    }
    #[tokio::test]
    async fn register_test_users() {
        init_env_logger(log::LevelFilter::Info, log::LevelFilter::Error).await;
        let mut app = create_test_service!();
        //  setup_test!(&app, true);
        let admin_token = TestHelpers::admin_login().await;
        let test_users = TestHelpers::load_test_users_from_config();

        for user in test_users.iter() {
            let req = actix_web::test::TestRequest::post()
                .uri("/auth/api/v1/users/register-test-user")
                .append_header((header::CONTENT_TYPE, "application/json"))
                .append_header((GameHeader::TEST, TestContext::as_json(true)))
                .append_header((GameHeader::PASSWORD, "password".to_string()))
                .append_header(("Authorization", admin_token.clone()))
                .set_json(&user)
                .to_request();
            let response = actix_web::test::call_service(&mut app, req).await;
            let status = response.status();
            let body = test::read_body(response).await;
            let service_response_as_json =
                std::str::from_utf8(&body).expect("should convert to str");
            if !status.is_success() {
                log::trace!("{} already registered", user.display_name.clone());
                assert_eq!(status, StatusCode::CONFLICT);
                log::trace!("Response body: {:?}", service_response_as_json);

                let sr: ServiceResponse = serde_json::from_str(&service_response_as_json)
                    .expect("failed to deserialize into ServiceResponse");

                assert_eq!(sr.status, StatusCode::CONFLICT);
            } else {
                //  we get back a service response with a client user in the body

                let client_user: ClientUser =
                    ServiceResponse::to_client_user(service_response_as_json)
                        .expect("Service Response should deserialize")
                        .1;

                let pretty_json = serde_json::to_string_pretty(&client_user)
                    .expect("Failed to pretty-print JSON");

                // Check if the pretty-printed JSON contains any underscores
                assert!(
                    !pretty_json.contains('_'),
                    "JSON contains an underscore character"
                );

                log::trace!("registered client_user: {:#?}", pretty_json);
            }
        }
    }

    pub struct TestHelpers {}
    impl TestHelpers {
        pub async fn admin_login() -> String {
            let profile = TestHelpers::load_admin_profile_from_config();

            let admin_pwd = env::var("ADMIN_PASSWORD")
                .expect("ADMIN_PASSWORD not found in environment - unable to continue");

            let request_context = RequestContext::new(&None, &None, &SERVICE_CONFIG);

            let auth_token = match login(&profile.email, &admin_pwd, &request_context).await {
                Ok(sr) => sr.get_token(),
                Err(_) => {
                    register(&admin_pwd, &profile, &request_context)
                        .await
                        .expect("registering a new user should work");
                    login(&profile.email, &admin_pwd, &request_context)
                        .await
                        .expect("login after register should work")
                        .get_token()
                }
            };

            auth_token.expect("should contain the admin auth token")
        }

        fn load_admin_profile_from_config() -> UserProfile {
            // Fetch the location from the environment variable
            let admin_json_path = env::var("ADMIN_PROFILE_JSON")
                .expect("ADMIN_PROFILE_JSON not found in environment - unable to continue");

            // Read the file
            let mut file = File::open(admin_json_path)
                .expect("if this fails, update ADMIN_PROFILE_JSON to point to the right file");
            let mut contents = String::new();
            file.read_to_string(&mut contents)
                .expect("This should not fail.");

            // Deserialize the JSON string into UserProfile
            let profile = serde_json::from_str::<UserProfile>(&contents).expect(
                "This should deserialize.  if it fails, make sure the JSON is in PascalCase",
            );

            profile
        }

        fn load_test_users_from_config() -> Vec<UserProfile> {
            let test_users_path = env::var("TEST_USERS_JSON")
                .expect("TEST_USERS_JSON not found in environment - unable to continue");

            // Read the file
            let mut file = File::open(test_users_path)
                .expect("if this fails, update ADMIN_PROFILE_JSON to point to the right file");
            let mut contents = String::new();
            file.read_to_string(&mut contents)
                .expect("This should not fail.");

            // Deserialize the JSON string into UserProfile
            let profiles = match serde_json::from_str::<Vec<UserProfile>>(&contents) {
                Ok(v) => v,
                Err(e) => {
                    panic!("{:#?}", e);
                }
            };

            profiles
        }
    }
}
