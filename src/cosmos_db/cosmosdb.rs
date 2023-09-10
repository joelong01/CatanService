#![allow(dead_code)]
use crate::{
    log_and_return_azure_core_error, log_return_unexpected_server_error,
    shared::models::{
        ClientUser, ConfigEnvironmentVariables, GameError, PersistUser, ResponseType,
    },
};
use std::collections::HashMap;

/**
 *  this is the class that calls directly to CosmosDb --
 */
use crate::{log_return_err, shared::models::ServiceResponse};
use azure_core::error::{ErrorKind, Result as AzureResult};
use azure_data_cosmos::prelude::{
    AuthorizationToken, CollectionClient, CosmosClient, DatabaseClient, Query, QueryCrossPartition,
};

use async_trait::async_trait;
use futures::StreamExt;
use log::info;
/**
 *  we have 3 cosmos collections that we are currently using:  User, Profile, and (eventually) Game.
 *  this just makes sure we consistently use them throughout the code.
 */
#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub enum CosmosCollectionName {
    User,
    Profile,
    Game,
}

struct CosmosCollectionNameValues {
    pub name: CosmosCollectionName,
    pub value: &'static str,
}

static COLLECTION_NAME_VALUES: [CosmosCollectionNameValues; 3] = [
    CosmosCollectionNameValues {
        name: CosmosCollectionName::User,
        value: "User-Collection",
    },
    CosmosCollectionNameValues {
        name: CosmosCollectionName::Profile,
        value: "Profile-Collection",
    },
    CosmosCollectionNameValues {
        name: CosmosCollectionName::Game,
        value: "Game-Collection",
    },
];
#[async_trait]
pub trait UserDbTrait {
    async fn setupdb(&self) -> Result<(), ServiceResponse>;
    async fn list(&self) -> Result<Vec<PersistUser>, ServiceResponse>;
    async fn update_or_create_user(
        &self,
        user: &PersistUser,
    ) -> Result<ServiceResponse, ServiceResponse>;
    async fn delete_user(&self, unique_id: &str) -> Result<(), ServiceResponse>;
    async fn find_user_by_id(&self, val: &str) -> Result<Option<PersistUser>, ServiceResponse>;
    async fn find_user_by_email(&self, val: &str) -> Result<Option<PersistUser>, ServiceResponse>;
}

/**
 *  this is a convinient way to pass around meta data about CosmosDb.  UserDb will also expose methods for calling
 *  cosmos (see below)
 */
pub struct UserDb {
    client: Option<CosmosClient>,
    database: Option<DatabaseClient>,
    collection_clients: HashMap<CosmosCollectionName, CollectionClient>,
    database_name: String,
}

impl UserDb {
    pub fn new(is_test: bool, catan_env: &'static ConfigEnvironmentVariables) -> Self {
        let client = public_client(&catan_env.cosmos_account, &catan_env.cosmos_token);
        let database_name;
        if is_test {
            database_name = catan_env.cosmos_database_name.clone() + "-test";
        } else {
            database_name = catan_env.cosmos_database_name.clone();
        }

        let database = client.database_client(database_name.clone());
        let mut collection_clients: HashMap<CosmosCollectionName, CollectionClient> =
            HashMap::new();
        for item in &COLLECTION_NAME_VALUES {
            let collection_name: String;
            if is_test {
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
    async fn execute_query(
        &self,
        collection_name: CosmosCollectionName,
        query_string: &str,
    ) -> AzureResult<Vec<PersistUser>> {
        let mut users = Vec::new();
        let query = Query::new(query_string.to_string());
        let collection = self.collection_clients.get(&collection_name).unwrap();
        let mut stream = collection
            .query_documents(query)
            .query_cross_partition(QueryCrossPartition::Yes)
            .into_stream::<serde_json::Value>();
        //
        // this just matches what list does, but only returns the first one
        // we are getting an error right now, but nothing to indicate what the error is.
        while let Some(response) = stream.next().await {
            match response {
                Ok(response) => {
                    for doc in response.documents() {
                        let user: PersistUser = serde_json::from_value(doc.clone())?;
                        users.push(user);
                    }
                    return Ok(users); // return user if found
                }
                Err(e) => {
                    log_return_err!(e)
                }
            }
        }
        Err(azure_core::Error::new(ErrorKind::Other, "User not found")) // return error if user not found
    }
    fn collection_name(&self, col_type: &CosmosCollectionName) -> String {
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

/**
 *  this is the scruct that contains methods to manipulate cosmosdb.  the idea is to be able to write code like
 *
 *      let mut user_db = UserDb::new().await;
 *      user_db.connect();
 *      user_db.list();
 *      user_db.create(...)
 */

#[async_trait]
impl UserDbTrait for UserDb {
    /**
     *  setup the database to make the sample work.  NOTE:  this will DELETE the database first.  to call this:
     *
     *  let userdb = UserDb::new();
     *  userdb.setupdb()
     */
    async fn setupdb(&self) -> Result<(), ServiceResponse> {
        info!("Deleting existing database");

        match self.database.as_ref().unwrap().delete_database().await {
            Ok(..) => info!("\tDeleted {} database", self.database_name),
            Err(e) => {
                log_and_return_azure_core_error!(
                    e,
                    &format!("Error deleting database: {}", self.database_name)
                );
            }
        }

        info!("Creating new database");
        match self
            .client
            .as_ref()
            .unwrap()
            .create_database(self.database_name.to_string())
            .await
        {
            Ok(..) => info!("\tCreated database"),
            Err(e) => {
                log_and_return_azure_core_error!(
                    e,
                    &format!("Error creatting database: {}", self.database_name)
                );
            }
        }

        info!("Creating collections");
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
                    info!(
                        "\tCreated {} collection",
                        collection_client.collection_name()
                    );
                }
                Err(e) => {
                    log_and_return_azure_core_error!(
                        e,
                        &format!(
                            "Error creating collection: {}",
                            collection_client.collection_name()
                        )
                    );
                }
            }
        }
        Ok(())
    }
    /**
     *  this will return *all* (non paginated) Users in the collection
     */
    async fn list(&self) -> Result<Vec<PersistUser>, ServiceResponse> {
        let query = r#"SELECT * FROM c WHERE c.partitionKey=1"#;
        match self.execute_query(CosmosCollectionName::User, query).await {
            Ok(users) => Ok(users),
            Err(e) => log_and_return_azure_core_error!(e, &format!("error in list()")),
        }
    }

    /**
     *  an api that creates a user in the cosmosdb users collection. in this sample, we return
     *  the full User object in the body, giving the client the partitionKey and user id
     */
    async fn update_or_create_user(
        &self,
        user: &PersistUser,
    ) -> Result<ServiceResponse, ServiceResponse> {
        let collection = self
            .collection_clients
            .get(&CosmosCollectionName::User)
            .unwrap();

        log::trace!("{}", serde_json::to_string(&user).unwrap());
        match collection
            .create_document(user.clone())
            .is_upsert(true)
            .await
        {
            Ok(..) => match serde_json::to_string(&user) {
                Ok(..) => Ok(ServiceResponse::new(
                    "created",
                    StatusCode::CREATED,
                    ResponseType::ClientUser(ClientUser::from_persist_user(user)),
                    GameError::NoError(String::default()),
                )),
                Err(e) => log_return_unexpected_server_error!(
                    e,
                    "unexpected error serializing user struct"
                ),
            },
            Err(e) => log_and_return_azure_core_error!(e, "update_or_create_user"),
        }
    }
    /**
     *  delete the user with the unique id
     */
    async fn delete_user(&self, unique_id: &str) -> Result<(), ServiceResponse> {
        let collection = self
            .collection_clients
            .get(&CosmosCollectionName::User)
            .unwrap();

        let doc_client = match collection.document_client(unique_id, &1) {
            Ok(client) => client,
            Err(e) => log_and_return_azure_core_error!(e, "Failed to get document client"),
        };

        match doc_client.delete_document().await {
            Ok(..) => Ok(()),
            Err(e) => log_and_return_azure_core_error!(e, "delete_user"),
        }
    }
    /**
     *  an api that finds a user by the id in the cosmosdb users collection.
     */
    async fn find_user_by_id(&self, val: &str) -> Result<Option<PersistUser>, ServiceResponse> {
        let query = format!(r#"SELECT * FROM c WHERE c.id = '{}'"#, val);
        match self.execute_query(CosmosCollectionName::User, &query).await {
            Ok(users) => {
                if !users.is_empty() {
                    Ok(Some(users.first().unwrap().clone())) // clone is necessary because `first()` returns a reference
                } else {
                    Ok(None)
                }
            }
            Err(e) => {
                log_and_return_azure_core_error!(e, "find_user_by_id");
            }
        }
    }
    async fn find_user_by_email(&self, val: &str) -> Result<Option<PersistUser>, ServiceResponse> {
        let query = format!(r#"SELECT * FROM c WHERE c.user_profile.Email = '{}'"#, val);
        match self.execute_query(CosmosCollectionName::User, &query).await {
            Ok(users) => {
                if !users.is_empty() {
                    log::trace!("found user with email={}", val);
                    Ok(Some(users.first().unwrap().clone()))
                } else {
                    Ok(None) // No user was found
                }
            }
            Err(e) => {
                log_and_return_azure_core_error!(e, "find_user_by_id");
            }
        }
    }
}

#[cfg(test)]
mod tests {

    use crate::{
        init_env_logger,
        middleware::environment_mw::{RequestContext, CATAN_ENV},
        shared::{models::UserProfile, utility::get_id},
        user_service::users::verify_cosmosdb,
    };

    use super::*;
    use bcrypt::{hash, DEFAULT_COST};
    use log::trace;
    #[tokio::test]

    async fn test_e2e() {
        // test_db_e2e(Some(TestContext {
        //     use_cosmos_db: true,
        // }))

        let context = RequestContext::test_default(true);
        test_db_e2e(&context).await;
    }
    pub async fn test_db_e2e(request_context: &RequestContext) {
        let user_db = &request_context.database;
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
                Ok(..) => trace!("created user {}", user.user_profile.email),
                Err(e) => panic!("failed to create user.  err: {}", e),
            }
        }

        // update a user

        let mut modified_user = users[0].clone();
        modified_user.validated_email = true;
        let _ = user_db
            .update_or_create_user(&modified_user)
            .await
            .expect("update should succeed");
        let test_user = user_db
            .find_user_by_id(&modified_user.id)
            .await
            .expect("should find this user")
            .expect("should be Some");
        assert!(test_user.validated_email);

        // find user by email
        log::trace!("looking for user {}", test_user.user_profile.email);
        let found_user = match user_db
            .find_user_by_email(&test_user.user_profile.email)
            .await
        {
            Ok(user) => user,
            Err(service_response) => {
                panic!(
                    "Error looking up by email: {}",
                    serde_json::to_string(&service_response).unwrap()
                );
            }
        };

        assert_eq!(
            found_user.unwrap().user_profile.email,
            test_user.user_profile.email
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
            let u = user_db
                .find_user_by_id(&first_user.id)
                .await
                .expect("find_user_by_id should not fail");
            match u {
                Some(found_user) => {
                    trace!("found user with email: {}", found_user.user_profile.email)
                }
                None => panic!("failed to find user that we just inserted"),
            }
        } else {
            panic!("the list should not be empty since we just filled it up!")
        }
        
       //  delete all the users
        for user in users {
            let result = user_db.delete_user(&user.id).await;
            match result {
                Ok(_) => {
                    trace!("deleted user with email: {}", &user.user_profile.email);
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
                phone_code: None,
            };

            users.push(user);
        }

        users
    }
}
use reqwest::StatusCode;
#[cfg(test)]
pub use tests::test_db_e2e;
