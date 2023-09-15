#[cfg(test)]
pub mod test {
    #![allow(dead_code)]
    use actix_http::StatusCode;
    use actix_web::test;

    use crate::middleware::request_context_mw::{RequestContext, TestContext};
    use crate::middleware::security_context::SecurityContext;
    use crate::middleware::service_config::SERVICE_CONFIG;
    use crate::user_service::users::{login, register};
    use crate::{
        shared::shared_models::{GameError, ResponseType, ServiceResponse, UserProfile},
        test::test_proxy::TestProxy,
    };
    use actix_web::{
        body::{BoxBody, EitherBody},
        Error,
    };
    use std::io::prelude::*;
    use std::{env, fs::File};
    use actix_http::Request;
    use actix_service::Service;
    use actix_web::dev::ServiceResponse as ActixServiceResponse;
    use crate::{create_app, create_test_service, init_env_logger};

    #[tokio::test]
    async fn test_new_proxy() {
        crate::init_env_logger(log::LevelFilter::Info, log::LevelFilter::Error).await;
        let app = test::init_service(create_app!()).await;
        let mut test_proxy = TestProxy::new(&app, None);

        let admin_profile = TestHelpers::load_admin_profile_from_config();
        let admin_pwd = env::var("ADMIN_PASSWORD")
            .expect("ADMIN_PASSWORD not found in environment - unable to continue");

        let service_response = test_proxy.login(&admin_profile.email, &admin_pwd).await;

        let auth_token = service_response
            .get_token()
            .expect("should contain auth token");
        assert!(auth_token.len() > 0);

        test_proxy.set_auth_token(&Some(auth_token));
        let service_response = test_proxy.get_profile().await;
        let client_user = service_response
            .to_client_user()
            .expect("this should be a client_user");

        assert!(client_user.id.len() > 0);
        assert_eq!(client_user.user_profile.email, admin_profile.email);

        
      
        //
        // clean up test user in production system
        let test_users = TestHelpers::load_test_users_from_config();
        assert!(test_users.len() > 0);
        for user in test_users {
            assert_ne!(user.email, admin_profile.email);
            // note that test context is not set -- so if we made a mistake (ahem) and put the test users in the 
            // production database, this will delete all of them.
            test_proxy.set_auth_token(&None);
            let result = test_proxy.login(&user.email, "password").await;

            if let Some(test_auth_token) = result.get_token() {
                test_proxy.set_auth_token(&Some(test_auth_token));
                let cu = test_proxy
                    .get_profile()
                    .await
                    .to_client_user()
                    .expect("this shoudl be there since login worked");
                let sr = test_proxy.delete_user(&cu).await;
                assert!(sr.status.is_success());
            }
        }

        let users = test_proxy
        .get_all_users()
        .await
        .get_client_users()
        .expect("there should be at least one user always (the admin)");

        assert!(users.len() > 0);

    }

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
    async fn register_test_users_test() {
        init_env_logger(log::LevelFilter::Info, log::LevelFilter::Error).await;
        let app = create_test_service!();
        let mut proxy = TestProxy::new(&app, None);
        //  setup_test!(&app, true);
        let admin_token = TestHelpers::admin_login().await;
        proxy.set_auth_token(&Some(admin_token));
        register_test_users(&proxy).await;

        
    }

    async fn register_test_users<S>(proxy: &TestProxy<'_, S>) // Add the expected lifetime and generic type for the function signature
    where
        S: Service<Request, Response = ActixServiceResponse<EitherBody<BoxBody>>, Error = Error> + 'static,
    {
        let test_users = TestHelpers::load_test_users_from_config();

        for user in test_users.iter() {
            let service_response = proxy.register_test_user(user, "password").await;

            if service_response.status.is_success() {
                //  we get back a service response with a client user in the body

                let client_user = service_response.to_client_user();

                let pretty_json = serde_json::to_string_pretty(&client_user)
                    .expect("Failed to pretty-print JSON");

                // Check if the pretty-printed JSON contains any underscores
                assert!(
                    !pretty_json.contains('_'),
                    "JSON contains an underscore character"
                );

                log::trace!("registered client_user: {:#?}", pretty_json);
            } else {
                log::trace!("{} already registered", user.display_name.clone());
                assert_eq!(service_response.status, StatusCode::CONFLICT);
            }
        }
    }

    pub struct TestHelpers {}
    impl TestHelpers {
        pub async fn admin_login() -> String {
            let profile = TestHelpers::load_admin_profile_from_config();

            let admin_pwd = env::var("ADMIN_PASSWORD")
                .expect("ADMIN_PASSWORD not found in environment - unable to continue");

            let request_context = RequestContext::new(
                &None,
                &None,
                &SERVICE_CONFIG,
                &SecurityContext::cached_secrets(),
            );

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
