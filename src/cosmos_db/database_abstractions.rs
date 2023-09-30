#![allow(dead_code)]
#![allow(unused_variables)]
use crate::middleware::request_context_mw::TestContext;
use crate::middleware::service_config::ServiceConfig;
use crate::shared::service_models::{PersistGame, PersistUser};
use crate::shared::shared_models::ServiceError;
use async_trait::async_trait;
use super::cosmosdb::CosmosDb;
use super::mocked_db::TestDb;

/**
 *  we have 3 cosmos collections that we are currently using:  User, Profile, and (eventually) Game.
 *  this just makes sure we consistently use them throughout the code.
 */
#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub enum CosmosDocType {
    User,
    Profile,
    Game,
}

pub struct CosmosCollectionNameValues {
    pub name: CosmosDocType,
    pub value: &'static str,
}

pub static COLLECTION_NAME_VALUES: [CosmosCollectionNameValues; 3] = [
    CosmosCollectionNameValues {
        name: CosmosDocType::User,
        value: "Users-Collection",
    },
    CosmosCollectionNameValues {
        name: CosmosDocType::Profile,
        value: "Profile-Collection",
    },
    CosmosCollectionNameValues {
        name: CosmosDocType::Game,
        value: "Game-Collection",
    },
];
#[async_trait]
pub trait GameDbTrait: Send + Sync {
    async fn load_game(&self, game_id: &str) -> Result<Vec<u8>, ServiceError>;
    async fn delete_games(&self, game_id: &str) -> Result<(), ServiceError>;
    async fn update_game_data(
        &self,
        game_id: &str,
        to_write: &PersistGame,
    ) -> Result<(), ServiceError>;
}

#[async_trait]
pub trait UserDbTrait: Send + Sync {
     async fn setupdb(&self) -> Result<(), ServiceError>;
    async fn list(&self) -> Result<Vec<PersistUser>, ServiceError>;
    async fn update_or_create_user(
        &self,
        user: &PersistUser,
    ) -> Result<(), ServiceError>;
    async fn delete_user(&self, unique_id: &str) -> Result<(), ServiceError>;
    async fn find_user_by_id(&self, val: &str) -> Result<PersistUser, ServiceError>;
    async fn find_user_by_email(&self, val: &str) -> Result<PersistUser, ServiceError>;
    async fn get_connected_users(
        &self,
        connected_user_id: &str,
    ) -> Result<Vec<PersistUser>, ServiceError>;

    fn get_collection_names(&self, is_test: bool) -> Vec<String> {
        COLLECTION_NAME_VALUES
            .iter()
            .map(|name_value| {
                if is_test {
                    format!("{}-Test", name_value.value)
                } else {
                    name_value.value.to_string()
                }
            })
            .collect()
    }
}
pub enum Database {
    Cosmos(CosmosDb),
    Test(TestDb),
}

#[async_trait]
impl UserDbTrait for Database {
    async fn setupdb(&self) -> Result<(), ServiceError> {
        match self {
            Database::Cosmos(db) => db.setupdb().await,
            Database::Test(db) => db.setupdb().await,
        }
    }

    async fn list(&self) -> Result<Vec<PersistUser>, ServiceError> {
        match self {
            Database::Cosmos(db) => db.list().await,
            Database::Test(db) => db.list().await,
        }
    }

    async fn update_or_create_user(
        &self,
        user: &PersistUser,
    ) -> Result<(), ServiceError> {
        match self {
            Database::Cosmos(db) => db.update_or_create_user(user).await,
            Database::Test(db) => db.update_or_create_user(user).await,
        }
    }

    async fn delete_user(&self, unique_id: &str) -> Result<(), ServiceError> {
        match self {
            Database::Cosmos(db) => db.delete_user(unique_id).await,
            Database::Test(db) => db.delete_user(unique_id).await,
        }
    }

    async fn find_user_by_id(&self, val: &str) -> Result<PersistUser, ServiceError> {
        match self {
            Database::Cosmos(db) => db.find_user_by_id(val).await,
            Database::Test(db) => db.find_user_by_id(val).await,
        }
    }

    async fn find_user_by_email(&self, val: &str) -> Result<PersistUser, ServiceError> {
        match self {
            Database::Cosmos(db) => db.find_user_by_email(val).await,
            Database::Test(db) => db.find_user_by_email(val).await,
        }
    }

    async fn get_connected_users(
        &self,
        connected_user_id: &str,
    ) -> Result<Vec<PersistUser>, ServiceError> {
        match self {
            Database::Cosmos(db) => db.get_connected_users(connected_user_id).await,
            Database::Test(db) => db.get_connected_users(connected_user_id).await,
        }
    }

    fn get_collection_names(&self, is_test: bool) -> Vec<String> {
        match self {
            Database::Cosmos(db) => db.get_collection_names(is_test),
            Database::Test(db) => db.get_collection_names(is_test),
        }
    }
}


#[async_trait]
impl GameDbTrait for Database {
    async fn load_game(&self, game_id: &str) -> Result<Vec<u8>, ServiceError> {
        match self {
            Database::Cosmos(db) => db.load_game(game_id).await,
            Database::Test(db) => db.load_game(game_id).await,
        }
    }

    async fn delete_games(&self, game_id: &str) -> Result<(), ServiceError> {
        match self {
            Database::Cosmos(db) => db.delete_games(game_id).await,
            Database::Test(db) => db.delete_games(game_id).await,
        }
    }

    async fn update_game_data(
        &self,
        game_id: &str,
        to_write: &PersistGame,
    ) -> Result<(), ServiceError> {
        match self {
            Database::Cosmos(db) => db.update_game_data(game_id, to_write).await,
            Database::Test(db) => db.update_game_data(game_id, to_write).await,
        }
    }
}

pub struct DatabaseWrapper {
    db: Box<Database>,
}

impl DatabaseWrapper {
    pub fn new_cosmos(is_test: bool, service_config: &'static ServiceConfig) -> Self {
        DatabaseWrapper {
            db:  Box::new(Database::Cosmos(CosmosDb::new(is_test, service_config))),
        }
    }

    pub fn new_test() -> Self {
        DatabaseWrapper {
            db: Box::new(Database::Test(TestDb::new())),
        }
    }

    pub fn new(test_context: &Option<TestContext>, service_config: &'static ServiceConfig) -> Self {
        match test_context {
            Some(context) => {
                if context.use_cosmos_db {
                    DatabaseWrapper::new_cosmos(true, service_config)
                } else {
                    DatabaseWrapper::new_test()
                }
            },
            None => DatabaseWrapper::new_cosmos(false, service_config),
        }
    }
    

    pub fn as_user_db(&self) -> &dyn UserDbTrait {
        &*self.db
    }

    pub fn as_game_db(&self) -> &dyn GameDbTrait {
        &*self.db
    }
}
