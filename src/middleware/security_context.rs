#![allow(dead_code)]
use crate::{
    azure_setup::azure_wrapper::{key_vault_get_secret, key_vault_save_secret},
    shared::service_models::Claims,
};

use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};

use super::service_config::SERVICE_CONFIG;

lazy_static::lazy_static! {
static ref SECRETS_CACHE: Arc<RwLock<SecurityContext>>= Arc::new(RwLock::new(SecurityContext::new()));}
use jsonwebtoken::{
    decode, encode, Algorithm, DecodingKey, EncodingKey, Header, TokenData, Validation,
};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct KeyKind;

impl KeyKind {
    pub const PRIMARY_KEY: &'static str = "primary-login-key";
    pub const SECONDARY_KEY: &'static str = "secondary-login-key";
    pub const TEST_PRIMARY_KEY: &'static str = "test-primary-login-key";
    pub const TEST_SECONDARY_KEY: &'static str = "test-secondary-login-key";
    pub const VALIDATATION_PRIMARY_KEY: &'static str = "validation-primary-key";
    pub const VALIDATATION_SECONDARY_KEY: &'static str = "validation-secondary-key";
}
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize, Debug)]
pub struct KeySet {
    pub primary_key_name: String,
    pub secondary_key_name: String,
    pub primary_key: String,
    pub secondary_key: String,
}

impl KeySet {
    pub fn new(p_key_name: &'static str, s_key_name: &'static str) -> Self {
        Self {
            primary_key_name: p_key_name.to_string(),
            secondary_key_name: s_key_name.to_string(),
            primary_key: Self::get_secret(p_key_name).to_owned(),
            secondary_key: Self::get_secret(s_key_name).to_owned(),
        }
    }
    // this needs work -- as it is, if we can't talk to KeyVault, or if the secret doesn't exists, this will create a temp
    // secret and use it, and then try to save it. that makes it easy to develop, but a bad production idea.
    // if key_vault_get_secret fails, this  should probably panic!.  we should also as part of initial provisioning
    // guarantee that keys are there.
    fn get_secret(name: &'static str) -> String {
        let secret = match key_vault_get_secret(&SERVICE_CONFIG.kv_name, name) {
            Ok(s) => s,
            Err(_) => {
                let new_key = SecurityContext::generate_jwt_key();
                if key_vault_save_secret(&SERVICE_CONFIG.kv_name, name, &new_key).is_err() {
                    log::error!(
                        "Failed to save secret in key vault.  all secrets signed with this \
                     key will be invalid when the process exits."
                    )
                }
                new_key
            }
        };
        secret
    }

    pub fn create_jwt_token(&self, claims: &Claims) -> Result<String, Box<dyn std::error::Error>> {
        let token_result = encode(
            &Header::new(Algorithm::HS512),
            &claims,
            &EncodingKey::from_secret(self.primary_key.as_ref()),
        );

        token_result.map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }

    pub fn sign_claims(&self, claims: &Claims) -> Result<String, Box<dyn std::error::Error>> {
        let token_result = encode(
            &Header::new(Algorithm::HS512),
            &claims,
            &EncodingKey::from_secret(self.primary_key.as_ref()),
        );

        token_result.map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }

    pub fn validate_token(&self, token: &str) -> Option<Claims> {
        // Try to validate with primary key first.
        let claims = match self.validate_jwt_token_with_key(&token, &self.primary_key) {
            Some(claims) => Some(claims.claims),
            None => {
                // If primary fails, try to validate with secondary key.
                match self.validate_jwt_token_with_key(&token, &self.secondary_key) {
                    Some(claims) => Some(claims.claims),
                    None => None,
                }
            }
        };
        claims
    }
    pub fn validate_jwt_token_with_key(
        &self,
        token: &str,
        secret_key: &str,
    ) -> Option<TokenData<Claims>> {
        let validation = Validation::new(Algorithm::HS512);
        match decode::<Claims>(
            &token,
            &DecodingKey::from_secret(secret_key.as_ref()),
            &validation,
        ) {
            Ok(c) => {
                Some(c) // or however you want to handle a valid token
            }
            Err(_) => None,
        }
    }
}

#[derive(Serialize, Deserialize, Eq, PartialEq, Debug, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct SecurityContext {
    pub login_keys: KeySet,
    pub validation_keys: KeySet,
    pub test_keys: KeySet,
}

impl SecurityContext {
    const SECURITY_CONTEXT_SECRET_NAME: &'static str = "security-context-secrets";
    pub fn cached_secrets() -> SecurityContext {
        let secrets = SECRETS_CACHE
            .read()
            .expect("cache should exists and read lock should always be acquired");
        secrets.clone()
    }

    pub(crate) fn new() -> Self {
        match key_vault_get_secret(&SERVICE_CONFIG.kv_name, Self::SECURITY_CONTEXT_SECRET_NAME) {
            Ok(json) => {
                match serde_json::from_str::<SecurityContext>(&json) {
                    Ok(sc) => sc,
                    Err(e) => {
                        log::error!("Failed to deserialize the security context: {}", e);
                        Self::create_and_save_security_context()
                    }
                }
            }
            Err(e) => {
                log::error!("Failed to retrieve secret from key vault: {}", e);
                Self::create_and_save_security_context()
            }
        }
    }
    
    fn create_and_save_security_context() -> SecurityContext {
        let security_context = SecurityContext {
            login_keys: KeySet::new(KeyKind::PRIMARY_KEY, KeyKind::SECONDARY_KEY),
            validation_keys: KeySet::new(
                KeyKind::VALIDATATION_PRIMARY_KEY,
                KeyKind::VALIDATATION_SECONDARY_KEY,
            ),
            test_keys: KeySet::new(KeyKind::TEST_PRIMARY_KEY, KeyKind::TEST_SECONDARY_KEY),
        };
    
        match serde_json::to_string(&security_context) {
            Ok(secrets) => {
                if let Err(e) = key_vault_save_secret(&SERVICE_CONFIG.kv_name, Self::SECURITY_CONTEXT_SECRET_NAME, &secrets) {
                    log::error!("Failed to save secret in key vault: {}", e);
                }
            },
            Err(e) => log::error!("Failed to serialize the security context: {}", e),
        }
    
        security_context
    }
    

    pub fn refresh_cache() {
        let security_context = SecurityContext {
            login_keys: KeySet::new(KeyKind::PRIMARY_KEY, KeyKind::SECONDARY_KEY),
            validation_keys: KeySet::new(
                KeyKind::VALIDATATION_PRIMARY_KEY,
                KeyKind::VALIDATATION_SECONDARY_KEY,
            ),
            test_keys: KeySet::new(KeyKind::TEST_PRIMARY_KEY, KeyKind::TEST_SECONDARY_KEY),
        };

        // Acquire the write lock and update the cache
        let mut cache = SECRETS_CACHE
            .write()
            .expect("Failed to acquire write lock on SECRETS_CACHE");
        *cache = security_context;
    }

    pub fn generate_jwt_key() -> String {
        let mut key = [0u8; 96]; // 96 bytes * 8 bits/byte = 768 bits.
        rand::thread_rng().fill_bytes(&mut key);
        openssl::base64::encode_block(&key)
    }
}
