/**
 *  this is the class that calls directly to CosmosDb --
 */
use crate::log_return_err;
use crate::middleware::environment_mw::RequestEnvironmentContext;
use crate::shared::models::User;
use azure_core::error::{ErrorKind, Result as AzureResult};
use azure_data_cosmos::prelude::{
    AuthorizationToken, CollectionClient, CosmosClient, DatabaseClient, Query, QueryCrossPartition,
};
use futures::StreamExt;
use log::info;
/**
 *  this is a convinient way to pass around meta data about CosmosDb.  UserDb will also expose methods for calling
 *  cosmos (see below)
 */
pub struct UserDb {
    client: Option<CosmosClient>,
    database: Option<DatabaseClient>,
    users_collection: Option<CollectionClient>,
    collection_name: String,
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
        let collection_name = context.env.container_name.clone();

        let database = client.database_client(database_name.clone());
        let collection = database.collection_client(collection_name.clone());

        Self {
            client: Some(client),
            database: Some(database),
            users_collection: Some(collection),
            database_name,
            collection_name,
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
        match self
            .database
            .as_ref()
            .unwrap()
            // note: this is where the field for the partion key is set -- if you change anything, make sure this is
            // a member of your document struct!
            .create_collection(self.collection_name.to_string(), "/partition_key")
            .await
        {
            Ok(..) => {
                info!("\tCreated {} collection", self.collection_name);
                Ok(())
            }
            Err(e) => log_return_err!(e),
        }
    }
    /**
     *  this will return *all* (non paginated) Users in the collection
     */
    pub async fn list(&self) -> AzureResult<Vec<User>> {
        let query = r#"SELECT * FROM c WHERE c.partition_key=1"#;
        match self.execute_query(query).await {
            Ok(users) => Ok(users),
            Err(e) => log_return_err!(e),
        }
    }
    /**
     * Execute an arbitrary query against the user database and return a list of users
     */
    async fn execute_query(&self, query_string: &str) -> AzureResult<Vec<User>> {
        let mut users = Vec::new();
        let query = Query::new(query_string.to_string());

        let mut stream = self
            .users_collection
            .as_ref()
            .unwrap()
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
                        let user: User = serde_json::from_value(doc.clone())?;
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
     *  the full User object in the body, giving the client the partition_key and user id
     */
    pub async fn create_user(&self, user: User) -> AzureResult<()> {
        match self
            .database
            .as_ref()
            .unwrap()
            .collection_client(self.collection_name.to_string())
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
        let collection = self.users_collection.as_ref().unwrap();
        let doc_client = collection.document_client(unique_id, &1)?;
        match doc_client.delete_document().await {
            Ok(..) => Ok(()),
            Err(e) => log_return_err!(e),
        }
    }
    /**
     *  an api that finds a user by the id in the cosmosdb users collection.
     */
    pub async fn find_user(&self, prop: &str, val: &str) -> AzureResult<User> {
        let query = format!(r#"SELECT * FROM c WHERE c.{} = '{}'"#, prop, val);
        match self.execute_query(&query).await {
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

    use crate::shared::utility::get_id;

    use super::*;
    use log::trace;

    #[tokio::test]
    async fn test_e2e() {
        let context = RequestEnvironmentContext::create(true);
        env_logger::init();
        let _ = env_logger::builder().is_test(true).try_init();

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
            match user_db.create_user(user_clone).await {
                Ok(..) => trace!("created user {}", user.email),
                Err(e) => panic!("failed to create user.  err: {}", e),
            }
        }

        // get a list of all users
        let users: Vec<User> = match user_db.list().await {
            Ok(u) => {
                trace!("all_users returned success");
                u
            }
            Err(e) => panic!("failed to setup database and collection {}", e),
        };

        if let Some(first_user) = users.first() {
            let u = user_db
                .find_user("id", first_user.id.as_ref().unwrap())
                .await;
            match u {
                Ok(found_user) => trace!("found user with email: {}", found_user.email),
                Err(e) => panic!("failed to find user that we just inserted. error: {}", e),
            }
        } else {
            panic!("the list should not be empty since we just filled it up!")
        }
        //
        //  delete all the users
        for user in users {
            let result = user_db.delete_user(&user.id.unwrap()).await;
            match result {
                Ok(_) => {
                    trace!("deleted user with email: {}", &user.email);
                }
                Err(e) => {
                    panic!("failed to delete user. error: {:#?}", e)
                }
            }
        }

        // get the list of users again -- should be empty
        let users: Vec<User> = match user_db.list().await {
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

    fn create_users() -> Vec<User> {
        let mut users = Vec::new();

        for i in 1..=5 {
            let user = User {
                partition_key: Some(1),
                id: Some(get_id()),
                password_hash: None,
                password: Some(format!("long_password_that_is_ a test {}", i)),
                email: format!("test{}@example.com", i),
                first_name: format!("Test{}", i),
                last_name: format!("User{}", i),
                display_name: format!("Test User{}", i),
                picture_url: format!("https://example.com/pic{}.jpg", i),
                foreground_color: format!("#00000{}", i),
                background_color: format!("#FFFFFF{}", i),
                games_played: Some(10 * i as u16),
                games_won: Some(5 * i as u16),
            };

            users.push(user);
        }

        users
    }
}
