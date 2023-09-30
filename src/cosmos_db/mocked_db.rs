#![allow(dead_code)]
#![allow(unused_variables)]
use std::{collections::HashMap, sync::Arc};

use crate::{
    log_return_bad_id, new_not_found_error,
    shared::{
        service_models::{PersistUser, PersistGame},
        shared_models::{GameError, ResponseType, ServiceError},
    }
};
use async_trait::async_trait;
use log::trace;
use reqwest::StatusCode;
use tokio::sync::RwLock;

use crate::cosmos_db::database_abstractions::{UserDbTrait, GameDbTrait};
lazy_static::lazy_static! {
    // Initialize singleton lobby instance
    static ref MOCKED_DB: Arc<TestDb> = Arc::new(TestDb::new());
}

pub struct TestDb {
    pub users: Arc<RwLock<HashMap<String, PersistUser>>>,
    pub games: Arc<RwLock<HashMap<String, PersistGame>>>,
}
impl TestDb {
    pub fn new() -> Self {
        Self {
            users: Arc::new(RwLock::new(HashMap::new())),
            games: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}
#[async_trait]
impl GameDbTrait for TestDb {

    async fn load_game(&self, game_id: &str) -> Result<Vec<u8>, ServiceError> {
        todo!();
    }
    async fn delete_games(&self, game_id: &str) -> Result<(), ServiceError> {
        todo!();
    }
    async fn update_game_data(&self, game_id: &str, to_write: &PersistGame)-> Result<(), ServiceError> {
        todo!();
    }
}
#[async_trait]
impl UserDbTrait for TestDb {
    
    async fn setupdb(&self) -> Result<(), ServiceError> {
        MOCKED_DB.users.write().await.clear();
        Ok(())
    }

    async fn list(&self) -> Result<Vec<PersistUser>, ServiceError> {
        Ok(MOCKED_DB.users.read().await.values().cloned().collect())
    }

    async fn update_or_create_user(
        &self,
        user: &PersistUser,
    ) -> Result<(), ServiceError> {
        let mut map = MOCKED_DB.users.write().await;
        let result = map.insert(user.id.clone(), user.clone());
        match result {
            None => trace!("user id {} added", user.id.clone()),
            Some(_) => trace!("user id {} updated", user.id.clone()),
        }
        Ok(())
    }

    async fn delete_user(&self, unique_id: &str) -> Result<(), ServiceError> {
        match MOCKED_DB.users.write().await.remove(unique_id) {
            Some(_) => Ok(()),
            None => {
                log_return_bad_id!(unique_id, "testdb::delete_user");
            }
        }
    }

    async fn find_user_by_id(&self, id: &str) -> Result<PersistUser, ServiceError> {
        match MOCKED_DB
            .users
            .read()
            .await
            .iter()
            .find(|(_key, user)| *user.id == *id)
        {
            Some(u) => Ok(u.1.clone()),
            None => return new_not_found_error!("Not Found"),
        }
    }
    async fn get_connected_users(
        &self,
        connected_user_id: &str,
    ) -> Result<Vec<PersistUser>, ServiceError> {
        let mut local_profiles = Vec::new();
    
        // Collect the values into a Vec
        let profiles: Vec<PersistUser> = MOCKED_DB.users.read().await.values().cloned().collect();
    
        for profile in profiles {
            if let Some(id) = &profile.connected_user_id {
                if *id == *connected_user_id {
                    local_profiles.push(profile);
                }
            }
        }
    
        Ok(local_profiles)
    }
    
    

    async fn find_user_by_email(&self, val: &str) -> Result<PersistUser, ServiceError> {
        match MOCKED_DB.users.read().await.iter().find(|(_key, user)| {
            // Access email through the pii field
            match &user.user_profile.pii {
                Some(pii) => &pii.email == val,
                None => false,
            }
        }) {
            Some(u) => Ok(u.1.clone()),
            None => new_not_found_error!("Not Found"),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{cosmos_db::cosmosdb::test_db_e2e, middleware::request_context_mw::RequestContext};

    #[tokio::test]

    async fn test_e2e() {
        // test_db_e2e(Some(TestContext {
        //     use_cosmos_db: false,
        // }))
        // .await;

        let context = RequestContext::test_default(false);
        test_db_e2e(&context).await;
    }
}
