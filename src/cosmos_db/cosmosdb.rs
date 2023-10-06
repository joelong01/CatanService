#![allow(dead_code)]
#![allow(unused_variables)]
use crate::{
    full_info,
    middleware::service_config::ServiceConfig,
    shared::service_models::{PersistGame, PersistUser},
    shared::shared_models::{GameError, ResponseType},
};
use std::collections::HashMap;

/**
 *  this is the class that calls directly to CosmosDb --
 */
use crate::shared::shared_models::ServiceError;
use actix_http::StatusCode;

use azure_data_cosmos::prelude::{
    AuthorizationToken, CollectionClient, CosmosClient, DatabaseClient, Query, QueryCrossPartition,
};

use async_trait::async_trait;
use futures::StreamExt;

use crate::cosmos_db::database_abstractions::{
    CosmosDocType, GameDbTrait, UserDbTrait, COLLECTION_NAME_VALUES,
};

/**
 *  this is a convinient way to pass around meta data about CosmosDb.  UserDb will also expose methods for calling
 *  cosmos (see below)
 */

#[derive(Clone)]
pub struct CosmosDb {
    client: Option<CosmosClient>,
    database: Option<DatabaseClient>,
    collection_clients: HashMap<CosmosDocType, CollectionClient>,
    database_name: String,
}

impl CosmosDb {
    pub fn new(use_test_collection: bool, service_config: &'static ServiceConfig) -> Self {
        let client = public_client(&service_config.cosmos_account, &service_config.cosmos_token);
        let database_name;
        if use_test_collection {
            database_name = service_config.cosmos_database_name.clone() + "-test";
        } else {
            database_name = service_config.cosmos_database_name.clone();
        }

        let database = client.database_client(database_name.clone());

        //
        //  we have array of name/value pairs for the document types we support and its corresponding name in cosmosdb
        //  here we loop over that array and crate a map of doc type -> cosmos_db collection names and then use the
        //  name to create a CollectionClient.  Then we add it to docType -> CollectionClient map so that everywhere
        //  else in the code we can do a lookup based on the kind of document we are accessing to the CollectionClient
        //  needed to manipulate it in CosmosDb

        let mut collection_clients: HashMap<CosmosDocType, CollectionClient> = HashMap::new();
        for item in &COLLECTION_NAME_VALUES {
            let collection_name: String;
            if use_test_collection {
                collection_name = format!("{}-test", item.value);
            } else {
                collection_name = item.value.to_owned();
            }
            let client = database.collection_client(collection_name);
            collection_clients.insert(item.name, client); // now we have a map of (say) CosmosCollectionName::User to "User-db-test"
        }

        Self {
            client: Some(client),
            database: Some(database),
            collection_clients,
            database_name,
        }
    }
    /**
     * Execute an arbitrary query against the user database and return a list of users
     */
    async fn execute_query<T: serde::de::DeserializeOwned>(
        &self,
        collection_name: CosmosDocType,
        query_string: &str,
    ) -> Result<Vec<T>, ServiceError> {
        let mut documents: Vec<T> = Vec::new();
        let query = Query::new(query_string.to_string());
        let collection = self.collection_clients.get(&collection_name).unwrap();
        let mut stream = collection
            .query_documents(query)
            .query_cross_partition(QueryCrossPartition::Yes)
            .into_stream::<serde_json::Value>();

        while let Some(response) = stream.next().await {
            match response {
                Ok(response) => {
                    for doc_value in response.documents() {
                        match serde_json::from_value::<T>(doc_value.clone()) {
                            Ok(doc) => documents.push(doc),
                            Err(e) => {
                                log::error!("Failed to deserialize document: {}", e);
                                return Err(ServiceError::new_json_error(
                                    &format!(
                                        "Failed to deserialize object for query: {}",
                                        query_string,
                                    ),
                                    &e,
                                ));
                            }
                        }
                    }
                }
                Err(e) => {
                    return Err(ServiceError::new_database_error(
                        "stream::next failed",
                        &format!("{:#?}", e),
                    ));
                }
            }
        }
        Ok(documents) // NOTE: These might be empty!
    }

    fn collection_name(&self, col_type: &CosmosDocType) -> String {
        let collection_client = self
            .collection_clients
            .get(col_type)
            .expect("this should be set in ::new");

        collection_client.collection_name().to_string()
    }
}

/**
 *  We only use the public client in this sample.
 *
 *  there are other sample out there that do ::from_resource() for the auth token.  To set this token, do to the
 *  Azure portal and pick your CosmosDb, then pick your "Keys" on the left pane.  You'll see a page that shows
 *  secrets -- "PRIMARY KEY", "SECONDARY KEY", etc.  Click on the eye for the SECONDARY KEY so you see the content
 *  in clear text, and then copy it when the devsecrets.sh script asks for the Cosmos token.  That key needs to
 *  be converted to base64 using primary_from_base64()
 */
fn public_client(account: &str, token: &str) -> CosmosClient {
    let auth_token = match AuthorizationToken::primary_from_base64(token) {
        Ok(token) => token,
        Err(e) => panic!("Failed to create authorization token: {}", e),
    };

    CosmosClient::new(account, auth_token)
}

#[async_trait]
impl GameDbTrait for CosmosDb {
    async fn load_game(&self, game_id: &str) -> Result<PersistGame, ServiceError> {
        let query = format!(r#"SELECT * FROM c WHERE c.id = '{}'"#, game_id);
        let persist_games: Vec<PersistGame> =
            self.execute_query(CosmosDocType::Game, &query).await?;
        if !persist_games.is_empty() {
            assert!(persist_games.len() == 1);
            Ok(persist_games.first().unwrap().clone()) // clone is necessary because `first()` returns a reference
        } else {
            Err(ServiceError::new_not_found("load_game", game_id))
        }
    }

    async fn delete_games(&self, game_id: &str) -> Result<(), ServiceError> {
        todo!()
    }
    async fn update_game_data(
        &self,
        game_id: &str,
        to_write: &PersistGame,
    ) -> Result<(), ServiceError> {
        let collection = self.collection_clients.get(&CosmosDocType::Game).unwrap();

        match collection
            .create_document(to_write.clone())
            .is_upsert(true)
            .await
        {
            Ok(..) => Ok(()),
            Err(e) => {
                return Err(ServiceError::new(
                    "unexpected serde serlialization error",
                    StatusCode::INTERNAL_SERVER_ERROR,
                    ResponseType::ErrorInfo(format!("Error: {}", e)),
                    GameError::HttpError,
                ))
            }
        }
    }
}

/**
 *  this is the scruct that contains methods to manipulate cosmosdb.  the idea is to be able to write code like
 *
 *      let mut user_db = UserDb::new().await;
 *      user_db.connect();
 *      user_db.list();
 *      user_db.create(...)
 */

#[async_trait]
impl UserDbTrait for CosmosDb {
    /**
     *  setup the database to make the sample work.  NOTE:  this will DELETE the database first.  to call this:
     *
     *  let userdb = UserDb::new();
     *  userdb.setupdb()
     */
    async fn setupdb(&self) -> Result<(), ServiceError> {
        full_info!("Deleting existing database");

        match self.database.as_ref().unwrap().delete_database().await {
            Ok(..) => full_info!("\tDeleted {} database", self.database_name),
            Err(e) => {
                return Err(ServiceError::new_database_error(
                    &format!("Error deleting database: {}", self.database_name),
                    &format!("{:#?}", e),
                ));
            }
        }

        full_info!("Creating new database");
        match self
            .client
            .as_ref()
            .unwrap()
            .create_database(self.database_name.to_string())
            .await
        {
            Ok(..) => full_info!("\tCreated database"),
            Err(e) => {
                return Err(ServiceError::new_database_error(
                    &format!("Error creating database: {}", self.database_name),
                    &format!("{:#?}", e),
                ));
            }
        }

        full_info!("Creating collections");
        for collection_client in self.collection_clients.values() {
            match self
                .database
                .as_ref()
                .unwrap()
                // note: this is where the field for the partion key is set -- if you change anything, make sure this is
                // a member of your document struct!
                .create_collection(collection_client.collection_name(), "/partitionKey")
                .await
            {
                Ok(..) => {
                    full_info!(
                        "\tCreated {} collection",
                        collection_client.collection_name()
                    );
                }
                Err(e) => {
                    return Err(ServiceError::new_database_error(
                        &format!(
                            "Error deleting collection: {}",
                            collection_client.collection_name()
                        ),
                        &format!("{:#?}", e),
                    ));
                }
            }
        }
        Ok(())
    }
    /**
     *  this will return *all* (non paginated) Users in the collection
     */
    async fn list(&self) -> Result<Vec<PersistUser>, ServiceError> {
        let query = r#"SELECT * FROM c WHERE c.partitionKey=1"#;
        let users = self.execute_query(CosmosDocType::User, query).await?;
        Ok(users)
    }

    /**
     *  an api that creates a user in the cosmosdb users collection. in this sample, we return
     *  the full User object in the body, giving the client the partitionKey and user id
     */
    async fn update_or_create_user(
        &self,
        user: &PersistUser,
    ) -> Result<(), ServiceError> {
        let collection = self.collection_clients.get(&CosmosDocType::User).unwrap();

        log::trace!("{}", serde_json::to_string(&user).unwrap());
        match collection
            .create_document(user.clone())
            .is_upsert(true)
            .await
        {
            Ok(..) => match serde_json::to_string(&user) {
                Ok(..) => Ok(()),
                Err(e) => {
                    return Err(ServiceError::new(
                        "unexpected serde serlialization error",
                        StatusCode::INTERNAL_SERVER_ERROR,
                        ResponseType::ErrorInfo(format!("Error: {}", e)),
                        GameError::HttpError,
                    ))
                }
            },
            Err(e) => {
                return Err(ServiceError::new_database_error(
                    "update_or_create_user",
                    &format!("{:#?}", e),
                ));
            }
        }
    }
    /**
     *  delete the user with the unique id
     */
    async fn delete_user(&self, unique_id: &str) -> Result<(), ServiceError> {
        let collection = self.collection_clients.get(&CosmosDocType::User).unwrap();

        let doc_client = match collection.document_client(unique_id, &1) {
            Ok(client) => client,
            Err(e) => {
                return Err(ServiceError::new_database_error(
                    "Failed to get document client",
                    &format!("{:#?}", e),
                ));
            }
        };

        match doc_client.delete_document().await {
            Ok(..) => Ok(()),
            Err(e) => Err(ServiceError::new_database_error(
                "delete_user",
                &format!("{:#?}", e),
            )),
        }
    }
    /**
     *  an api that finds a user by the id in the cosmosdb users collection.
     */
    async fn find_user_by_id(&self, val: &str) -> Result<PersistUser, ServiceError> {
        let query = format!(r#"SELECT * FROM c WHERE c.id = '{}'"#, val);
        match self
            .execute_query::<PersistUser>(CosmosDocType::User, &query)
            .await
        {
            Ok(users) => {
                if let Some(user) = users.first() {
                    full_info!("user: {:#?}", user.id);
                    Ok(user.clone()) // clone is necessary because `first()` returns a reference
                } else {
                    Err(ServiceError::new_not_found("not found", val))
                }
            }
            Err(e) => Err(ServiceError::new_database_error(
                "find_user_by_id",
                &format!("{:#?}", e),
            )),
        }
    }

    async fn get_connected_users(
        &self,
        connected_user_id: &str,
    ) -> Result<Vec<PersistUser>, ServiceError> {
        let query = format!(
            r#"SELECT * FROM c WHERE c.connected_user_id = '{}'"#,
            connected_user_id
        );
        Ok(self.execute_query(CosmosDocType::User, &query).await?)
    }
    async fn find_user_by_email(&self, val: &str) -> Result<PersistUser, ServiceError> {
        let query = format!(
            r#"SELECT * FROM c WHERE c.user_profile.Pii.Email = '{}'"#,
            val
        );
        let users = self
            .execute_query::<PersistUser>(CosmosDocType::User, &query)
            .await?;

        if !users.is_empty() {
            Ok(users.first().unwrap().clone())
        } else {
             Err(ServiceError::new_not_found("not found", val))
        }
    }
}

#[cfg(test)]
pub mod tests {

    use crate::{
        init_env_logger,
        middleware::{request_context_mw::RequestContext, service_config::SERVICE_CONFIG},
        shared::{
            service_models::Role,
            shared_models::{PersonalInformation, UserProfile, UserType},
        },
        user_service::users::verify_cosmosdb,
    };

    use super::*;
    use bcrypt::{hash, DEFAULT_COST};
    use log::trace;
    #[tokio::test]

    pub async fn test_e2e() {
        let context = RequestContext::test_default(true);
        test_db_e2e(&context).await;
    }
    pub async fn test_db_e2e(request_context: &RequestContext) {
        let user_db = request_context.database.as_user_db();
        init_env_logger(log::LevelFilter::Trace, log::LevelFilter::Error).await;
        verify_cosmosdb(&request_context)
            .await
            .expect("azure should be configured to run these tests");
        // create the database -- note this will DELETE the database as well
        match user_db.setupdb().await {
            Ok(..) => trace!("created test db and collection"),
            Err(e) => panic!("failed to setup database and collection {}", e),
        }
        // create users and add them to the database
        let users = create_users().await;
        for user in users.clone() {
            match user_db.update_or_create_user(&user).await {
                Ok(..) => trace!("created user {}", user.user_profile.get_email_or_panic()),
                Err(e) => panic!("failed to create user.  err: {}", e),
            }
        }

        // update a user

        let mut modified_user = users[0].clone();
        modified_user.user_profile.validated_email = true;
        let _ = user_db
            .update_or_create_user(&modified_user)
            .await
            .expect("update should succeed");
        let test_user = user_db
            .find_user_by_id(&modified_user.id)
            .await
            .expect("should find this user");

        assert!(test_user.user_profile.validated_email);

        // find user by email
        log::trace!(
            "looking for user {}",
            test_user.user_profile.get_email_or_panic()
        );
        let found_user = match user_db
            .find_user_by_email(&test_user.user_profile.get_email_or_panic())
            .await
        {
            Ok(user) => user,
            Err(service_error) => {
                panic!(
                    "Error looking up by email: {}",
                    serde_json::to_string(&service_error).unwrap()
                );
            }
        };

        assert_eq!(
            found_user.user_profile.get_email_or_panic(),
            test_user.user_profile.get_email_or_panic()
        );

        // get a list of all users
        let users: Vec<PersistUser> = match user_db.list().await {
            Ok(u) => {
                trace!("all_users returned success");
                u
            }
            Err(e) => panic!("failed to setup database and collection {}", e),
        };

        if let Some(first_user) = users.first() {
            let found_user = user_db
                .find_user_by_id(&first_user.id)
                .await
                .expect("find_user_by_id should not fail");
            trace!(
                "found user with email: {}",
                found_user.user_profile.get_email_or_panic()
            );
        }

        //  delete all the users
        for user in users {
            let result = user_db.delete_user(&user.id).await;
            match result {
                Ok(_) => {
                    trace!(
                        "deleted user with email: {}",
                        &user.user_profile.get_email_or_panic()
                    );
                }
                Err(e) => {
                    panic!("failed to delete user. error: {:#?}", e)
                }
            }
        }

        //  get the list of users again -- should be empty
        let users: Vec<PersistUser> = match user_db.list().await {
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
    pub async fn create_users() -> Vec<PersistUser> {
        let mut users = Vec::new();
        for i in 1..=5 {
            let password = format!("long_password_that_is_ a test {}", i);
            let password_hash = hash(&password, DEFAULT_COST).unwrap();
            let pii = PersonalInformation {
                email: format!("test{}@example.com", i),
                phone_number: SERVICE_CONFIG.test_phone_number.to_owned(),
                first_name: format!("Test{}", i),
                last_name: format!("User{}", i),
            };
            let user = PersistUser {
                partition_key: 1,
                id: PersistUser::new_id(),
                password_hash: Some(password_hash.to_owned()),
                user_profile: UserProfile {
                    user_type: UserType::Connected,
                    pii: Some(pii),
                    display_name: format!("Test User{}", i),
                    picture_url: format!("https://example.com/pic{}.jpg", i),
                    foreground_color: format!("#00000{}", i),
                    background_color: format!("#FFFFFF{}", i),
                    text_color: format!("0000000"),
                    games_played: Some(10 * i as u16),
                    games_won: Some(5 * i as u16),
                    user_id: None,
                    validated_email: false,
                    validated_phone: false,
                },

                phone_code: None,
                roles: vec![Role::User, Role::TestUser],
                connected_user_id: None,
            };

            users.push(user);
        }

        users
    }
}
