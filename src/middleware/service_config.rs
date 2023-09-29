#![allow(dead_code)]

use std::{collections::HashMap, env};

/**
 *  this file contains the middleware that injects ServiceContext into the Request.  The data in RequestContext is the
 *  configuration data necessary for the Service to run -- the secrets loaded from the environment, hard coded strings,
 *  etc.
 *
 */

use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};

use crate::full_info;


// load the environment variables once and only once the first time they are accessed (which is in main() in this case)
lazy_static! {
    pub static ref SERVICE_CONFIG: ServiceConfig =
        ServiceConfig::load_from_env().unwrap();
}

/**
 *  the .devcontainer/required-secrets.json contains the list of secrets needed to run this application.  this stuctu
 *  holds them so that they are more convinient to use
 */
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ServiceConfig {
    pub resource_group: String,
    pub kv_name: String,
    pub azure_location: String,

    pub admin_email: String,

    pub cosmos_token: String,
    pub cosmos_account: String,
    pub cosmos_database_name: String,


    pub ssl_key_location: String,
    pub ssl_cert_location: String,
    pub login_secret_key: String,
    pub validation_secret_key: String,

    pub rust_log: String,

    pub test_phone_number: String,
    pub service_phone_number: String,

    pub test_email: String,
    pub service_email: String,

    pub name_value_map: HashMap<String, String>,
}
fn insert_env_to_map(name_map: &mut HashMap<String, String>, env_var_name: &str) -> anyhow::Result<String> {
    let value = env::var(env_var_name).expect(&format!(
        "{} not found in environment - unable to continue",
        env_var_name
    ));
    name_map.insert(value.clone(), format!("${}", env_var_name));
    Ok(value)
}
impl ServiceConfig {
    pub fn load_from_env() -> anyhow::Result<Self> {
        let mut name_map = HashMap::new();

        let resource_group = insert_env_to_map(&mut name_map, "AZURE_RESOURCE_GROUP")?;
        let kv_name = insert_env_to_map(&mut name_map, "KEV_VAULT_NAME")?;
        let cosmos_token = insert_env_to_map(&mut name_map, "COSMOS_AUTH_TOKEN")?;
        let cosmos_account = insert_env_to_map(&mut name_map, "COSMOS_ACCOUNT_NAME")?;
        let cosmos_database = insert_env_to_map(&mut name_map, "COSMOS_DATABASE_NAME")?;
        let ssl_key_location = insert_env_to_map(&mut name_map, "SSL_KEY_FILE")?;
        let ssl_cert_location = insert_env_to_map(&mut name_map, "SSL_CERT_FILE")?;
        let login_secret_key = insert_env_to_map(&mut name_map, "LOGIN_SECRET_KEY")?;
        let validation_secret_key = insert_env_to_map(&mut name_map, "VALIDATION_SECRET_KEY")?;
        let rust_log = insert_env_to_map(&mut name_map, "RUST_LOG")?;
        let test_phone_number = insert_env_to_map(&mut name_map, "TEST_PHONE_NUMBER")?;
        let service_phone_number = insert_env_to_map(&mut name_map, "SERVICE_PHONE_NUMBER")?;
        let test_email = insert_env_to_map(&mut name_map, "TEST_EMAIL")?;
        let service_email = insert_env_to_map(&mut name_map, "SERVICE_FROM_EMAIL")?;
        let location = insert_env_to_map(&mut name_map, "AZURE_LOCATION")?;
        let admin_email = insert_env_to_map(&mut name_map, "ADMIN_EMAIL")?;
        Ok(Self {
            resource_group,
            kv_name,
            test_phone_number,
            service_phone_number,
            azure_location: location,
            cosmos_token,
            cosmos_account,
            ssl_key_location,
            ssl_cert_location,
            login_secret_key,
            validation_secret_key,
            cosmos_database_name: cosmos_database,
            rust_log,
            test_email,
            service_email,
            name_value_map: name_map.clone(),
            admin_email,
        })
    }

    pub fn dump_values(&self) {
        full_info!("cosmos_token: {}", self.cosmos_token);
        full_info!("cosmos_account: {}", self.cosmos_account);
        full_info!("ssl_key_location: {}", self.ssl_key_location);
        full_info!("ssl_cert_location: {}", self.ssl_cert_location);
        full_info!("login_secret_key: {}", self.login_secret_key);
        full_info!("validation_secret_key: {}", self.validation_secret_key);
        full_info!("database_name: {}", self.cosmos_database_name);
        full_info!("rust_log: {}", self.rust_log);
        full_info!("kv_name: {}", self.kv_name);
        full_info!("test_phone_number: {}", self.test_phone_number);
        full_info!("test_email: {}", self.test_email);
        full_info!("service_mail: {}", self.service_email);
        full_info!("admin_email: {}", self.admin_email)
    }
}
impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            cosmos_token: String::default(),
            cosmos_account: "user-cosmos-account".to_owned(),
            ssl_key_location: String::default(),
            ssl_cert_location: String::default(),
            login_secret_key: String::default(),
            validation_secret_key: String::default(),
            cosmos_database_name: "Users-Database".to_owned(),
            rust_log: "actix_web=trace,actix_server=trace,rust=trace".to_owned(),
            kv_name: String::default(),
            test_phone_number: String::default(),
            resource_group: "catan-rg".to_owned(),
            azure_location: "westus3".to_owned(),
            service_phone_number: String::default(),
            test_email: String::default(),
            service_email: String::default(),
            name_value_map: HashMap::<String, String>::new(),
            admin_email: String::default(),
        }
    }
}