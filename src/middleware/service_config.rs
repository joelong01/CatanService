#![allow(dead_code)]

use std::{collections::HashMap, fs::File, io::Read};

use clap::Parser;
/**
 *  this file contains the middleware that injects ServiceContext into the Request.  The data in RequestContext is the
 *  configuration data necessary for the Service to run -- the secrets loaded from the environment, hard coded strings,
 *  etc.
 *
 */
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::Arguments;

// load the environment variables once and only once the first time they are accessed (which is in main() in this case)
lazy_static! {
    pub static ref SERVICE_CONFIG: ServiceConfig = {
        let args = Arguments::parse();
        ServiceConfig::from_file(&args.config_file).expect(&format!("Failed to load ServiceConfig from {}", args.config_file))
    };
}


/**
 *  the .devcontainer/required-secrets.json contains the list of secrets needed to run this application.  this stuctu
 *  holds them so that they are more convinient to use
 */
#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceConfig {
    pub admin_email: String,
    pub admin_password: String,
    pub admin_profile_json: String,
    pub azure_communication_connection_string: String,
    pub azure_location: String,
    pub resource_group: String,
    pub cosmos_account: String,
    pub cosmos_token: String,
    pub cosmos_database_name: String,
    pub host_name: String,
    pub key_vault_name: String,
    pub login_secret_key: String,
    pub rust_log: String,
    pub service_email: String,
    pub service_phone_number: String,
    pub ssl_cert_file: String,
    pub ssl_key_file: String,
    pub test_cred_cache_location: String,
    pub test_email: String,
    pub test_phone_number: String,
    pub test_users_json: String,
    pub validation_secret_key: String,
    #[serde(skip)]
    pub name_value_map: HashMap<String, String>,
     #[serde(skip)]
    pub config_file: String
}

impl ServiceConfig {
    pub fn from_file(config_file: &str) -> Result<Self, std::io::Error> {
        let mut file = File::open(config_file)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        let mut config: Self = serde_json::from_str(&contents).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Failed to deserialize: {}", e),
            )
        })?;

        let parsed: Value = serde_json::from_str(&contents)?;
        let mut name_value_map = HashMap::new();

        if let Value::Object(map) = &parsed {
            for (key, value) in map {
                if let Value::String(value_str) = value {
                    name_value_map.insert(value_str.clone(), key.clone());
                }
            }
        }

        config.name_value_map = name_value_map;
        config.config_file = config_file.to_string();

        Ok(config)
    }
}

impl Default for ServiceConfig {
    fn default() -> Self {
        ServiceConfig {
            admin_email: String::default(),
            admin_password: String::default(),
            admin_profile_json: String::default(),
            azure_communication_connection_string: String::default(),
            azure_location: String::default(),
            resource_group: String::default(),
            cosmos_account: String::default(),
            cosmos_token: String::default(),
            cosmos_database_name: String::default(),
            host_name: String::default(),
            key_vault_name: String::default(),
            login_secret_key: String::default(),
            rust_log: String::default(),
            service_email: String::default(),
            service_phone_number: String::default(),
            ssl_cert_file: String::default(),
            ssl_key_file: String::default(),
            test_cred_cache_location: String::default(),
            test_email: String::default(),
            test_phone_number: String::default(),
            test_users_json: String::default(),
            validation_secret_key: String::default(),
            name_value_map: HashMap::new(),
            config_file: String::default()
        }
    }
}