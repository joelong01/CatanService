#![allow(dead_code)]
use std::{collections::HashMap, sync::Arc};

use crate::shared::models::{GameError, PersistUser, ResponseType, ServiceResponse};
use azure_core::{
    error::{ErrorKind, Result as AzureResult},
    Error,
};
use log::trace;
use tokio::sync::RwLock;
lazy_static::lazy_static! {
    // Initialize singleton lobby instance
    static ref MOCKED_DB: Arc<TestDb> = Arc::new(TestDb::new());
}

pub struct TestDb {
    pub users: Arc<RwLock<HashMap<String, PersistUser>>>,
}
impl TestDb {
    pub fn new() -> Self {
        Self {
            users: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    pub async fn setupdb() -> AzureResult<()> {
        MOCKED_DB.users.write().await.clear();
        Ok(())
    }

    pub async fn list() -> AzureResult<Vec<PersistUser>> {
        Ok(MOCKED_DB.users.read().await.values().cloned().collect())
    }

    pub async fn update_or_create_user(user: &PersistUser) -> AzureResult<()> {
        let mut map = MOCKED_DB.users.write().await;
        let result = map.insert(user.id.clone(), user.clone());
        match result {
            None => trace!("user id {} added", user.id.clone()),
            Some(_) => trace!("user id {} updated", user.id.clone())
        }
        Ok(())
    }

    pub async fn delete_user(unique_id: &str) -> AzureResult<()> {
        match MOCKED_DB.users.write().await.remove(unique_id) {
            Some(_) => Ok(()),
            None => Err(Error::new(ErrorKind::MockFramework, "User Not found")),
        }
    }

    pub async fn find_user_by_id(id: &str) -> Result<PersistUser, ServiceResponse> {
        match MOCKED_DB
            .users
            .read()
            .await
            .iter()
            .find(|(_key, user)| *user.id == *id)
        {
            Some(u) => Ok(u.1.clone()),
            None => Err(ServiceResponse::new(
                "",
                reqwest::StatusCode::NOT_FOUND,
                ResponseType::NoData,
                GameError::BadId(id.to_owned()),
            )),
        }
    }

    pub async fn find_user_by_email(val: &str) -> Result<PersistUser, ServiceResponse> {
        match MOCKED_DB
            .users
            .read()
            .await
            .iter()
            .find(|(_key, user)| *user.user_profile.email == *val)
        {
            Some(u) => Ok(u.1.clone()),
            None => Err(ServiceResponse::new(
                "",
                reqwest::StatusCode::NOT_FOUND,
                ResponseType::NoData,
                GameError::BadId(val.to_owned()),
            )),
        }
    }
}

#[cfg(test)]
mod tests {

    use crate::{
        init_env_logger,
        middleware::environment_mw::CATAN_ENV,
        shared::{models::UserProfile, utility::get_id},
    };

    use super::*;
    use bcrypt::{hash, DEFAULT_COST};
    use log::trace;
    #[tokio::test]

    async fn test_e2e() {
        init_env_logger(log::LevelFilter::Trace, log::LevelFilter::Error).await;

        // create the database -- note this will DELETE the database as well
        match TestDb::setupdb().await {
            Ok(..) => trace!("created test db and collection"),
            Err(e) => panic!("failed to setup database and collection {}", e),
        }
        // create users and add them to the database
        let users = create_users();
        for user in users {
            match TestDb::update_or_create_user(&user).await {
                Ok(..) => trace!("created user {}", user.user_profile.email),
                Err(e) => panic!("failed to create user.  err: {}", e),
            }

            let result = TestDb::update_or_create_user(&user).await;
            assert!(result.is_err());
        }

        // try to create the same user again:

        // get a list of all users
        let users: Vec<PersistUser> = match TestDb::list().await {
            Ok(u) => {
                trace!("all_users returned success");
                u
            }
            Err(e) => panic!("failed to setup database and collection {}", e),
        };

        if let Some(first_user) = users.first() {
            let u = TestDb::find_user_by_id(&first_user.id).await;
            match u {
                Ok(found_user) => {
                    trace!("found user with email: {}", found_user.user_profile.email)
                }
                Err(e) => panic!("failed to find user that we just inserted. error: {:#?}", e),
            }
        } else {
            panic!("the list should not be empty since we just filled it up!")
        }
        //
        //  delete all the users
        for user in users {
            let result = TestDb::delete_user(&user.id).await;
            match result {
                Ok(_) => {
                    trace!("deleted user with email: {}", &user.user_profile.email);
                }
                Err(e) => {
                    panic!("failed to delete user. error: {:#?}", e)
                }
            }
        }

        // get the list of users again -- should be empty
        let users: Vec<PersistUser> = match TestDb::list().await {
            Ok(u) => {
                trace!("all_users returned success");
                u
            }
            Err(e) => panic!("failed to setup database and collection {}", e),
        };
        if users.len() != 0 {
            panic!("we deleted all the test users but list() found some!");
        }
    }

    fn create_users() -> Vec<PersistUser> {
        let mut users = Vec::new();

        for i in 1..=5 {
            let password = format!("long_password_that_is_ a test {}", i);
            let password_hash = hash(&password, DEFAULT_COST).unwrap();
            let user = PersistUser {
                partition_key: 1,
                id: get_id(),
                password_hash: Some(password_hash.to_owned()),
                user_profile: UserProfile {
                    email: format!("test{}@example.com", i),
                    first_name: format!("Test{}", i),
                    last_name: format!("User{}", i),
                    display_name: format!("Test User{}", i),
                    phone_number: CATAN_ENV.test_phone_number.to_owned(),
                    picture_url: format!("https://example.com/pic{}.jpg", i),
                    foreground_color: format!("#00000{}", i),
                    background_color: format!("#FFFFFF{}", i),
                    text_color: format!("0000000"),
                    games_played: Some(10 * i as u16),
                    games_won: Some(5 * i as u16),
                },
                validated_email: false,
                validated_phone: false,
            };

            users.push(user);
        }

        users
    }
}
