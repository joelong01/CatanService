#![allow(dead_code)]
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use azure_data_cosmos::CosmosEntity;
use serde::{Deserialize, Serialize};

use crate::middleware::request_context_mw::TestContext;

use super::shared_models::{ClientUser, UserProfile};
use uuid::Uuid;

/**
 *  Every CosmosDb document needs to define the partition_key.  In Rust we do this via this trait.
 */
impl CosmosEntity for PersistUser {
    type Entity = u64;

    fn partition_key(&self) -> Self::Entity {
        self.partition_key
    }
}

/**
 * this is the document stored in cosmosdb.  the "id" field and the "partition_key" field are "special" in that the
 * system needs them. if id is not specified, cosmosdb will create a guid for the id (and create an 'id' field), You
 * can partition on any value, but it should be something that works well with the partion scheme that cosmos uses.
 * for this sample, we assume the db size is small, so we just partion on a number that the sample always sets to 1
 * note:  you don't want to use camelCase or PascalCase for this as you need to be careful about how 'id' and 'partionKey'
 * are spelled and capitalized
 */

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]

pub struct PersistUser {
    pub id: String, // not set by client
    #[serde(rename = "partitionKey")]
    pub partition_key: u64, // the cosmos client seems to care about the spelling of both id and partitionKey
    pub local_user_owner_id: Option<String>,
    pub password_hash: Option<String>, // when it is pulled from Cosmos, the hash is set
    pub validated_email: bool,         // has the mail been validated?
    pub validated_phone: bool,         // has the phone number been validated?
    pub user_profile: UserProfile,
    pub phone_code: Option<String>,
    pub roles: Vec<Role>,
}

impl PersistUser {
    pub fn new() -> Self {
        Self {
            id: PersistUser::get_id(),
            local_user_owner_id: None,
            partition_key: 1,
            password_hash: None,
            user_profile: UserProfile::default(),
            validated_email: false,
            validated_phone: false,
            phone_code: None,
            roles: vec![Role::User],
        }
    }

    pub fn from_client_user(client_user: &ClientUser, hash: String) -> Self {
        Self {
            id: client_user.id.clone(),
            local_user_owner_id: None,
            partition_key: 1,
            password_hash: Some(hash.to_owned()),
            user_profile: client_user.user_profile.clone(),
            validated_email: false,
            validated_phone: false,
            phone_code: None,
            roles: vec![Role::User],
        }
    }
    pub fn from_user_profile(profile: &UserProfile, hash: String) -> Self {
        Self {
            id: match &profile.user_id {
                Some(identifier) => identifier.clone(),
                None => PersistUser::get_id(),
            },
            local_user_owner_id: profile.user_id.clone(),
            partition_key: 1,
            password_hash: Some(hash.clone()),
            user_profile: profile.clone(),
            validated_email: false,
            validated_phone: false,
            phone_code: None,
            roles: vec![Role::User],
        }
    }

    pub fn update_profile(&mut self, new_profile: &UserProfile) {
        self.user_profile.update_from(new_profile);
    }

    /// Generates a unique user ID.
    ///
    /// This function creates random user IDs by creating a guid
    ///
    /// # Returns
    ///
    /// * A unique `String` ID.
    pub fn get_id() -> String {
        Uuid::new_v4().to_string()
    }
}
impl Default for PersistUser {
    fn default() -> Self {
        Self::new()
    }
}

//
//  an enum of roles that a user can be in
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub enum Role {
    Admin,
    User,
    TestUser,
    Validation,
}

// DO NOT ADD A #[serde(rename_all = "PascalCase")] macro to this struct!
// it will throw an error and you'll spend hours figuring out why it doesn't work - the rust bcrypt library cares about
// capitalization and enforces standard claim names
#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Clone)]
pub struct Claims {
    pub id: String,
    pub sub: String,
    pub exp: usize,
    pub roles: Vec<Role>,
    pub test_context: Option<TestContext>,
}

impl Claims {
    pub fn new(
        id: &str,
        email: &str,
        duration_secs: u64,
        roles: &Vec<Role>,
        test_context: &Option<TestContext>,
    ) -> Self {
        let exp = ((SystemTime::now() + Duration::from_secs(duration_secs))
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()) as usize;
        Self {
            id: id.to_owned(),
            sub: email.to_owned(),
            exp,
            roles: roles.clone(),
            test_context: test_context.clone(),
        }
    }
}
