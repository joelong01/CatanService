#![allow(dead_code)]

pub mod test {

    use crate::middleware::request_context_mw::{RequestContext, SERVICE_CONFIG};
    use crate::shared::shared_models::UserProfile;
    use crate::user_service::users::{login, register};
    use std::io::prelude::*;
    use std::{env, fs::File};

    #[tokio::test]
    async fn test_get_auth_token() {
        TestHelpers::get_admin_auth_token().await;
        print!("ok");
    }

    pub struct TestHelpers {}
    impl TestHelpers {
        async fn get_admin_auth_token() -> String {
            let profile = TestHelpers::load_user_profile();

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

        fn load_user_profile() -> UserProfile {
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
    }
}
