use std::{collections::HashMap, fmt};

/**
 *  this is the class that calls directly to CosmosDb --
 */
use crate::{log_return_err};
use crate::middleware::environment_mw::RequestEnvironmentContext;
use crate::shared::models::PersistUser;
use azure_core::error::{ErrorKind, Result as AzureResult};
use azure_data_cosmos::prelude::{
    AuthorizationToken, CollectionClient, CosmosClient, DatabaseClient, Query, QueryCrossPartition,
};

use futures::StreamExt;
use log::info;

/**
 *  we have 3 cosmos container that we are currently using:  User, Profile, and (eventually) Game.
 *  this just makes sure we consistently use them throughout the code.
 */
#[derive(PartialEq, Eq, Hash)]
enum CosmosCollectionName {
    User,
    Profile,
    Game,
}

//
//  this converts them to string
impl fmt::Display for CosmosCollectionName {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            CosmosCollectionName::User => write!(f, "User-Collection"),
            CosmosCollectionName::Profile => write!(f, "Profile-Collection"),
            CosmosCollectionName::Game => write!(f, "Game-Collection"),
        }
    }
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
 *      user_db.connec();
 *      user_db.list();
 *      user_db.create(...)
 */

impl UserDb {
    pub async fn new(context: &RequestEnvironmentContext) -> Self {
        let client = public_client(&context.env.cosmos_account, &context.env.cosmos_token);
        let database_name = context.database_name.clone();
        let collection_names = vec![
            CosmosCollectionName::User,
            CosmosCollectionName::Profile,
            CosmosCollectionName::Game,
        ];
        let database = client.database_client(database_name.clone());
        let mut collection_clients: HashMap<CosmosCollectionName, CollectionClient> =
            HashMap::new();
        for name in collection_names {
            let client = database.collection_client(name.to_string());
            collection_clients.insert(name, client);
        }

        Self {
            client: Some(client),
            database: Some(database),
            collection_clients,
            database_name,
        }
    }

    /**
     *  setup the database to make the sample work.  NOTE:  this will DELETE the database first.  to call this:
     *
     *  let userdb = UserDb::new();
     *  userdb.setupdb()
     */
    pub async fn setupdb(&self) -> AzureResult<()> {
        info!("Deleting existing database");

        match self.database.as_ref().unwrap().delete_database().await {
            Ok(..) => info!("\tDeleted {} database", self.database_name),
            Err(e) => {
                if format!("{}", e).contains("404") {
                    info!("\tDatabase {} not found", self.database_name);
                } else {
                    log_return_err!(e)
                }
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
            Err(e) => log_return_err!(e),
        }

        info!("Creating collections");
        for name in self.collection_clients.keys() {
            match self
                .database
                .as_ref()
                .unwrap()
                // note: this is where the field for the partion key is set -- if you change anything, make sure this is
                // a member of your document struct!
                .create_collection(name.to_string(), "/partitionKey")
                .await
            {
                Ok(..) => {
                    info!("\tCreated {} collection", name);
                }
                Err(e) => log_return_err!(e),
            }
        }
        Ok(())
    }
    /**
     *  this will return *all* (non paginated) Users in the collection
     */
    pub async fn list(&self) -> AzureResult<Vec<PersistUser>> {
        let query = r#"SELECT * FROM c WHERE c.partitionKey=1"#;
        match self.execute_query(CosmosCollectionName::User, query).await {
            Ok(users) => Ok(users),
            Err(e) => log_return_err!(e),
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
                    info!("\n{:#?}", response);
                    for doc in response.documents() {
                        // Process the document
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

    /**
     *  an api that creates a user in the cosmosdb users collection. in this sample, we return
     *  the full User object in the body, giving the client the partitionKey and user id
     */
    pub async fn create_user(&self, user: PersistUser) -> AzureResult<()> {
        match self
            .database
            .as_ref()
            .unwrap()
            .collection_client(CosmosCollectionName::User.to_string())
            .create_document(user.clone())
            .await
        {
            Ok(..) => match serde_json::to_string(&user) {
                Ok(..) => Ok(()),
                Err(e) => Err(e.into()),
            },
            Err(e) => log_return_err!(e),
        }
    }
    /**
     *  delete the user with the unique id
     */
    pub async fn delete_user(&self, unique_id: &str) -> AzureResult<()> {
        let collection = self
            .collection_clients
            .get(&CosmosCollectionName::User)
            .unwrap();
        let doc_client = collection.document_client(unique_id, &1)?;
        match doc_client.delete_document().await {
            Ok(..) => Ok(()),
            Err(e) => log_return_err!(e),
        }
    }
    /**
     *  an api that finds a user by the id in the cosmosdb users collection.
     */
    pub async fn find_user_by_id(&self, val: &str) -> AzureResult<PersistUser> {
        let query = format!(r#"SELECT * FROM c WHERE c.id = '{}'"#, val);
        match self.execute_query(CosmosCollectionName::User, &query).await {
            Ok(users) => {
                if !users.is_empty() {
                    Ok(users.first().unwrap().clone()) // clone is necessary because `first()` returns a reference
                } else {
                    Err(azure_core::Error::new(ErrorKind::Other, "User not found"))
                }
            }
            Err(e) => log_return_err!(e),
        }
    }
    pub async fn find_user_by_profile(&self, prop: &str, val: &str) -> AzureResult<PersistUser> {
        let query = format!(
            r#"SELECT * FROM c WHERE c.userProfile.{} = '{}'"#,
            prop, val
        );
        match self.execute_query(CosmosCollectionName::User, &query).await {
            Ok(users) => {
                if !users.is_empty() {
                    Ok(users.first().unwrap().clone()) // clone is necessary because `first()` returns a reference
                } else {
                    Err(azure_core::Error::new(ErrorKind::Other, "User not found"))
                }
            }
            Err(e) => log_return_err!(e),
        }
    }
}
#[cfg(test)]
mod tests {

    use crate::{shared::{models::UserProfile, utility::get_id},init_env_logger};

    use super::*;
    use bcrypt::{hash, DEFAULT_COST};
    use log::trace;
    #[tokio::test]
    async fn test_e2e() {
        let context = RequestEnvironmentContext::create(true);
     
        init_env_logger().await;

        // create the database -- note this will DELETE the database as well
        let user_db = UserDb::new(&context).await;
        match user_db.setupdb().await {
            Ok(..) => trace!("created test db and collection"),
            Err(e) => panic!("failed to setup database and collection {}", e),
        }
        // create users and add them to the database
        let users = create_users();
        for user in users {
            let user_clone = user.clone();
            match user_db.create_user(user_clone.clone()).await {
                Ok(..) => trace!("created user {}", user.user_profile.email),
                Err(e) => panic!("failed to create user.  err: {}", e),
            }

            let result = user_db.create_user(user_clone.clone()).await;
            assert!(result.is_err());
        }

        // try to create the same user again:

        // get a list of all users
        let users: Vec<PersistUser> = match user_db.list().await {
            Ok(u) => {
                trace!("all_users returned success");
                u
            }
            Err(e) => panic!("failed to setup database and collection {}", e),
        };

        if let Some(first_user) = users.first() {
            let u = user_db.find_user_by_id(&first_user.id).await;
            match u {
                Ok(found_user) => {
                    trace!("found user with email: {}", found_user.user_profile.email)
                }
                Err(e) => panic!("failed to find user that we just inserted. error: {}", e),
            }
        } else {
            panic!("the list should not be empty since we just filled it up!")
        }
        //
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

        // get the list of users again -- should be empty
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
                    picture_url: format!("https://example.com/pic{}.jpg", i),
                    foreground_color: format!("#00000{}", i),
                    background_color: format!("#FFFFFF{}", i),
                    text_color: format!("0000000"),
                    games_played: Some(10 * i as u16),
                    games_won: Some(5 * i as u16),
                },
            };

            users.push(user);
        }

        users
    }
}
