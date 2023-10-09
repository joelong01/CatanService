#[cfg(test)]
pub mod test {
    #![allow(dead_code)]
    use actix_http::StatusCode;
    use actix_web::test;
    use tracing::info;

    use crate::middleware::request_context_mw::RequestContext;
    use crate::shared::shared_models::{
        GameError, ProfileStorage, ResponseType, ServiceError, UserProfile, LoginHeaderData
    };
    use crate::test::test_proxy::TestProxy;
    use crate::user_service::users::{login, register_user};
    use crate::{create_service, create_test_service, full_info, init_env_logger};
    use actix_http::Request;
    use actix_service::Service;
    use actix_web::dev::ServiceResponse as ActixServiceResponse;
    use actix_web::{
        body::{BoxBody, EitherBody},
        Error,
    };
    use std::io::prelude::*;
    use std::{env, fs::File};

    #[tokio::test]
    async fn test_new_proxy() {
        crate::init_env_logger(log::LevelFilter::Info, log::LevelFilter::Error).await;
        let test_service = test::init_service(create_service!()).await;
        let mut test_proxy = TestProxy::new(&test_service);

        let admin_auth_token = TestHelpers::admin_login().await;

        assert!(admin_auth_token.len() > 0);

        test_proxy.set_auth_token(Some(admin_auth_token.clone()));

        let client_user_profile = test_proxy
            .get_profile("Self")
            .await
            .expect("this should be a client_user");

        assert!(client_user_profile.user_id.unwrap().len() > 0);

        let count = TestHelpers::delete_all_test_users_from_config(&mut test_proxy).await;
        full_info!("deleted {} test users", count);
        test_proxy.set_auth_token(Some(admin_auth_token.clone()));

        let users = test_proxy
            .get_all_users()
            .await
            .expect("there should be at least one user always (the admin)");

        assert!(users.len() > 0);

        let test_users =
            TestHelpers::register_test_users(&mut test_proxy, Some(admin_auth_token)).await;
        for user in &test_users {
            test_proxy.set_auth_token(None);
            let auth_token = test_proxy
                .login(&LoginHeaderData::new(
                    &user.get_email_or_panic(),
                    "password",
                    ProfileStorage::CosmosDbTest,
                ))
                .await
                .expect("just registered accounts should be able to login");
            test_proxy.set_auth_token(Some(auth_token));
            let profile = test_proxy
                .get_profile("Self")
                .await
                .expect("to be able to get my own profile");
            assert!(profile.user_id.is_some());
        }
        assert!(test_users.len() > 0);
        let count = TestHelpers::delete_all_test_users_from_config(&mut test_proxy).await;
        full_info!("deleted {} test users", count);
    }

    #[tokio::test]
    async fn test_get_auth_token() {
        crate::init_env_logger(log::LevelFilter::Info, log::LevelFilter::Error).await;
        let _admin_token = TestHelpers::admin_login().await;
        let test_users = TestHelpers::load_test_users_from_config();
        log::trace!("{}", serde_json::to_string(&test_users).unwrap());
    }
    #[tokio::test]
    async fn test_service_response_serialization() {
        init_env_logger(log::LevelFilter::Trace, log::LevelFilter::Error).await;
        let sr = ServiceError::new(
            "already exists",
            StatusCode::ACCEPTED,
            ResponseType::NoData,
            GameError::NoError(String::default()),
        );

        let json = serde_json::to_string(&sr).unwrap();
        full_info!("to_http_response: {}", json);
        match serde_json::from_str::<ServiceError>(&json) {
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
        let mut proxy = TestProxy::new(&app);
        //  setup_test!(&app, true);

        let users = TestHelpers::register_test_users(&mut proxy, None).await;
        for user in users {
            assert!(user.pii.is_some());
            assert!(user.user_id.is_some());
            let auth_token = proxy
                .login(&LoginHeaderData::new(
                    &user.pii.unwrap().email,
                    "password",
                    ProfileStorage::CosmosDbTest,
                ))
                .await
                .expect("Login to succeed");
            assert!(auth_token.len() > 0);
            let profile = proxy
                .get_profile("Self")
                .await
                .expect("to get my own profile");
            assert!(profile.user_id.expect("a user_id").len() > 0);
        }
    }
    /**
     *  this test deletes all users in the Users-Collection-test Collection
     *  You cannot run this in parallel with any test that expects test users to be present.
     *  in general, i've found parallel tests with state requirements (e.g. the users in the db) are not
     *  compatible.
     */
    #[tokio::test]
    async fn delete_test_users() {
        init_env_logger(log::LevelFilter::Info, log::LevelFilter::Error).await;
        let app = create_test_service!();
        let mut proxy = TestProxy::new(&app);
        TestHelpers::delete_all_test_users(&mut proxy).await;
        //
        //  make sure that deleting empty works
        TestHelpers::delete_all_test_users(&mut proxy).await;
    }

    #[tokio::test]
    async fn profile_test() {
        init_env_logger(log::LevelFilter::Info, log::LevelFilter::Error).await;
        let app = create_test_service!();
        let mut test_proxy = TestProxy::new(&app);
        let admin_auth_token = TestHelpers::admin_login().await;

        assert!(admin_auth_token.len() > 0);

        test_proxy.set_auth_token(Some(admin_auth_token.clone()));

        let client_user_profile = test_proxy
            .get_profile("Self")
            .await
            .expect("this should be a client_user");
        full_info!(
            "admin id: {}",
            client_user_profile
                .user_id
                .expect("id to be Some in profile")
        );
    }

    pub struct TestHelpers {}
    impl TestHelpers {
        pub async fn admin_login() -> String {
            full_info!("logging in as admin");
            let profile = TestHelpers::load_admin_profile_from_config();

            let admin_pwd = env::var("ADMIN_PASSWORD")
                .expect("ADMIN_PASSWORD not found in environment - unable to continue");

            let request_context = RequestContext::admin_default(&profile);
            let login_data = LoginHeaderData {
                user_name: profile.get_email_or_panic().clone(),
                password: admin_pwd.clone(),
                profile_location: ProfileStorage::CosmosDb, // our admin is in the production collection
            };
            let auth_token = match login(&login_data, &request_context).await {
                Ok(token) => token,
                Err(_) => {
                    register_user(&admin_pwd, &profile, &request_context)
                        .await
                        .expect("registering a new user should work");
                    login(&login_data, &request_context)
                        .await
                        .expect("login after register should work")
                }
            };

            assert!(auth_token.len() > 0);
            auth_token
        }

        fn load_admin_profile_from_config() -> UserProfile {
            // Fetch the location from the environment variable
            let admin_json_path = env::var("ADMIN_PROFILE_JSON")
                .expect("ADMIN_PROFILE_JSON not found in environment - unable to continue");

            // Read the file
            let mut file = File::open(admin_json_path.clone())
                .expect("if this fails, update ADMIN_PROFILE_JSON to point to the right file");
            let mut contents = String::new();
            file.read_to_string(&mut contents)
                .expect("This should not fail.");

            // Deserialize the JSON string into UserProfile
            let profile = serde_json::from_str::<UserProfile>(&contents).expect(
                &format!("This should deserialize.  if it fails, make sure the Admin Profile at {} JSON is in camelCase", &admin_json_path)
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
        pub async fn register_test_users<S>(
            proxy: &mut TestProxy<'_, S>,
            admin_token: Option<String>,
        ) -> Vec<UserProfile>
        where
            S: Service<
                    Request,
                    Response = ActixServiceResponse<EitherBody<BoxBody>>,
                    Error = Error,
                > + 'static,
        {
            full_info!("registering test users");
            // Use the provided admin_token if it's Some, otherwise generate a new one
            let admin_token = if let Some(token) = admin_token {
                token
            } else {
                TestHelpers::admin_login().await
            };
            proxy.set_auth_token(Some(admin_token));

            let test_users = TestHelpers::load_test_users_from_config();
            let mut profiles = Vec::new();

            for user_profile in test_users.iter() {
                let result = proxy
                    .register_test_user(user_profile, "password")
                    .await;
                match result {
                    Ok(profile) => {
                        profiles.push(profile.clone());

                        let pretty_json = serde_json::to_string_pretty(&profile)
                            .expect("Failed to pretty-print JSON");

                        // Check if the pretty-printed JSON contains any underscores
                        assert!(
                            !pretty_json.contains('_'),
                            "JSON contains an underscore character"
                        );

                        log::trace!("registered client_user: {:#?}", pretty_json);
                    }
                    Err(service_error) => {
                        log::trace!("{} already registered", user_profile.display_name.clone());
                        assert_eq!(service_error.status, StatusCode::CONFLICT);
                        let email = user_profile.pii.clone().unwrap().email;
                        let profile = proxy.get_profile(&email).await.expect("should be there");
                        profiles.push(profile.clone());
                    }
                }
            }

            profiles
        }
        pub async fn delete_all_test_users<S>(proxy: &mut TestProxy<'_, S>) -> String
        // Add the expected lifetime and generic type for the function signature
        where
            S: Service<
                    Request,
                    Response = ActixServiceResponse<EitherBody<BoxBody>>,
                    Error = Error,
                > + 'static,
        {
            info!("deleting all test users");
            let admin_token = TestHelpers::admin_login().await;
            proxy.set_auth_token(Some(admin_token.clone()));
            let profiles = proxy
                .get_all_users()
                .await
                .expect("should at least be an empty vec!");

            for profile in profiles {
                proxy
                    .delete_user(&profile.user_id.unwrap())
                    .await
                    .expect("success");
            }

            let profiles = proxy
                .get_all_users()
                .await
                .expect("should at least be an empty vec!");
            assert!(profiles.len() == 0);
            admin_token
        }
        pub async fn delete_all_test_users_from_config<S>(
            test_proxy: &mut TestProxy<'_, S>,
        ) -> usize
        // Add the expected lifetime and generic type for the function signature
        where
            S: Service<
                    Request,
                    Response = ActixServiceResponse<EitherBody<BoxBody>>,
                    Error = Error,
                > + 'static,
        {
            let mut count = 0;
            let test_users = TestHelpers::load_test_users_from_config();
            assert!(test_users.len() > 0);
            for user in test_users {
                // note that test context is not set -- so if we made a mistake (ahem) and put the test users in the
                // production database, this will delete all of them.
                test_proxy.set_auth_token(None);
                let response = test_proxy
                    .login(&LoginHeaderData::new(
                        &user.get_email_or_panic(),
                        "password",
                        ProfileStorage::CosmosDbTest,
                    ))
                    .await;

                match response {
                    Ok(test_auth_token) => {
                        test_proxy.set_auth_token(Some(test_auth_token));
                        let profile = test_proxy
                            .get_profile("Self")
                            .await
                            .expect("this should be there since login worked");
                        let user_id = profile.user_id.expect("a logged in user must have an id!");
                        test_proxy.delete_user(&user_id).await.expect("success");
                        count = count + 1;
                    }
                    Err(e) => {
                        full_info!(
                            "could not delete {}.  code: {}",
                            user.get_email_or_panic(),
                            e.status
                        );
                    }
                }
            }
            count
        }
    }
}
