use azure_core::StatusCode;
/**
 * this is the module where I define the structures needed for the data in Cosmos
 */
use azure_data_cosmos::CosmosEntity;
use serde::{Deserialize, Serialize};
use strum_macros::Display;
use std::env;

use anyhow::{Context, Result};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, Display)]
pub enum GameError {
    PlayerMismatch,
    MissingPlayerId,
    IdNotFoundInOrder,
}

/**
 *  Every CosmosDb document needs to define the partition_key.  In Rust we do this via this trait.
 */
impl CosmosEntity for User {
    type Entity = u64;

    fn partition_key(&self) -> Self::Entity {
        self.partition_key.unwrap()
    }
}

/**
 * this is the document stored in cosmosdb.  the "id" field and the "partition_key" field are "special" in that the
 * system needs them. if id is not specified, cosmosdb will create a guild for the id (and create an 'id' field), You
 * can partition on any value, but it should be something that works well with the partion scheme that cosmos uses.
 * for this sample, we assume the db size is small, so we just partion on a number that the sample always sets to 1
 *
 */

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: Option<String>,            // not set by client
    pub partition_key: Option<u64>,    // Option<> so that the client can skip this
    pub password_hash: Option<String>, // when it is pulled from Cosmos, the hash is set
    pub password: Option<String>,      // when the user is registered, the password is set
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub display_name: String,
    pub picture_url: String,
    pub foreground_color: String,
    pub background_color: String,
    pub games_played: Option<u16>,
    pub games_won: Option<u16>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Credentials {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub id: String,
    pub sub: String,
    pub exp: usize,
}

/**
 *  We want every response to be in JSON format so that it is easier to script calling the service...when
 *  we don't have "natural" JSON (e.g. when we call 'setup'), we return the JSON of this object.
 */
#[derive(Debug, Serialize, Clone)]
pub struct ServiceResponse {
    pub message: String,
    pub status: StatusCode,
    pub body: String,
}

/**
 *  the .devcontainer/required-secrets.json contains the list of secrets needed to run this application.  this stuctu
 *  holds them so that they are more convinient to use
 */
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CatanEnvironmentVariables {
    pub cosmos_token: String,
    pub cosmos_account: String,
    pub ssl_key_location: String,
    pub ssl_cert_location: String,
    pub login_secret_key: String,
    pub database_name: String,
    pub container_name: String,
    pub rust_log: String,
}

impl CatanEnvironmentVariables {
    pub fn load_from_env() -> Result<Self> {
        let cosmos_token =
            env::var("COSMOS_AUTH_TOKEN").context("COSMOS_AUTH_TOKEN not found in environment")?;
        let cosmos_account = env::var("COSMOS_ACCOUNT_NAME")
            .context("COSMOS_ACCOUNT_NAME not found in environment")?;
        let ssl_key_location =
            env::var("SSL_KEY_FILE").context("SSL_KEY_FILE not found in environment")?;
        let ssl_cert_location =
            env::var("SSL_CERT_FILE").context("SSL_CERT_FILE not found in environment")?;
        let login_secret_key =
            env::var("LOGIN_SECRET_KEY").context("LOGIN_SECRET_KEY not found in environment")?;
        let database_name = env::var("CATAN_USER_DATABASE_NAME")
            .context("CATAN_USER_DATABASE_NAME not found in environment")?;
        let container_name = env::var("CATAN_USER_CONTAINER_NAME")
            .context("CATAN_USER_CONTAINER_NAME not found in environment")?;
        let rust_log = env::var("RUST_LOG").context("RUST_LOG not found in environment")?;

        Ok(Self {
            cosmos_token,
            cosmos_account,
            ssl_key_location,
            ssl_cert_location,
            login_secret_key,
            database_name,
            container_name,
            rust_log,
        })
    }
}
impl Default for CatanEnvironmentVariables {
    fn default() -> Self {
        Self {
            cosmos_token: "".to_owned(),
            cosmos_account: "".to_owned(),
            ssl_key_location: "".to_owned(),
            ssl_cert_location: "".to_owned(),
            login_secret_key: "".to_owned(),
            database_name: "Users-Database".to_owned(),
            container_name: "User-Container".to_owned(),
            rust_log: "actix_web=trace,actix_server=trace,rust=trace".to_owned(),
        }
    }
}
