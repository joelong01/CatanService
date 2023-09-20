#![allow(dead_code)]
use std::{collections::HashMap, sync::Arc};

use crate::{shared::{shared_models::{GameError, ResponseType, ServiceResponse, UserProfile}, service_models::PersistUser}, log_return_bad_id, new_not_found_error};
use async_trait::async_trait;
use log::trace;
use reqwest::StatusCode;
use tokio::sync::RwLock;

use super::cosmosdb::UserDbTrait;
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
}
#[async_trait]
impl UserDbTrait for TestDb {
    async fn setupdb(&self) -> Result<(), ServiceResponse> {
        MOCKED_DB.users.write().await.clear();
        Ok(())
    }

    async fn list(&self) -> Result<Vec<PersistUser>, ServiceResponse> {
        Ok(MOCKED_DB.users.read().await.values().cloned().collect())
    }

async fn update_or_create_user(&self, user: &PersistUser) -> Result<ServiceResponse, ServiceResponse> {
        let mut map = MOCKED_DB.users.write().await;
        let result = map.insert(user.id.clone(), user.clone());
        match result {
            None => trace!("user id {} added", user.id.clone()),
            Some(_) => trace!("user id {} updated", user.id.clone()),
        }
        Ok(ServiceResponse::new(
            "created",
            StatusCode::CREATED,
            ResponseType::Profile(UserProfile::from_persist_user(user)),
            GameError::NoError(String::default()),
        ))
    }

    async fn delete_user(&self, unique_id: &str) -> Result<(), ServiceResponse> {
        match MOCKED_DB.users.write().await.remove(unique_id) {
            Some(_) => Ok(()),
            None => {
                log_return_bad_id!(unique_id, "testdb::delete_user");
            },
        }
    }

    async fn find_user_by_id(&self, id: &str) -> Result<PersistUser, ServiceResponse> {
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


    async fn find_user_by_email(&self, val: &str) -> Result<PersistUser, ServiceResponse> {
        match MOCKED_DB
            .users
            .read()
            .await
            .iter()
            .find(|(_key, user)| {
                // Access email through the pii field
                match &user.user_profile.pii {
                    Some(pii) => &pii.email == val,
                    None => false,
                }
            })
        {
            Some(u) => Ok(u.1.clone()),
            None => new_not_found_error!("Not Found"),
        }
    }

}

#[cfg(test)]
mod tests {
    use crate::{
        cosmos_db::cosmosdb::test_db_e2e,
        middleware::request_context_mw::RequestContext,
    };

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
