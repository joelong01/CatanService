#![allow(dead_code)]
#![allow(unused_imports)]
use log::trace;
use once_cell::sync::Lazy;
use once_cell::sync::OnceCell;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::collections::HashSet;
use std::env;
use std::process::Command;
use std::str;
use std::sync::Mutex;
use tracing::info;

use lazy_static::lazy_static;

use crate::init_env_logger;
use crate::middleware::environment_mw::TestContext;
use crate::middleware::environment_mw::CATAN_ENV;
use crate::shared::models::Claims;
use crate::trace_function;
use crate::user_service::users::create_jwt_token;
use crate::user_service::users::generate_jwt_key;
use crate::user_service::users::validate_jwt_token;

static SUBSCRIPTION_ID: OnceCell<String> = OnceCell::new();
#[derive(Debug, Deserialize)]
pub struct CosmosSecretsOutput {
    #[serde(rename = "connectionStrings")]
    connection_strings: Vec<CosmosSecret>,
}

#[derive(Serialize, Deserialize, Eq, PartialEq, Clone, Debug)]
pub struct CosmosSecret {
    #[serde(rename = "connectionString")]
    pub connection_string: String,

    #[serde(rename = "description")]
    pub description: String,

    #[serde(rename = "keyKind")]
    pub key_kind: String,

    #[serde(rename = "type")]
    pub key_type: String,
}

impl CosmosSecret {
    pub fn equal_by_val(&self, other: &CosmosSecret) -> bool {
        if self.connection_string == other.connection_string
            && self.description == other.description
            && self.key_kind == other.key_kind
            && self.key_type == other.key_type
        {
            return true;
        }
        false
    }
}

pub fn verify_login_or_panic() -> String {
    // Check if the user is already logged into Azure
    if let Some(subscription_id) = SUBSCRIPTION_ID.get() {
        trace!("user already logged into azure");
        return subscription_id.clone();
    }

    let mut cmd = Command::new("az");
    cmd.arg("account").arg("show");

    match exec_os(&mut cmd) {
        Ok(output) => {
            let response: Value = serde_json::from_str(&output)
                .unwrap_or_else(|_| panic!("Failed to parse JSON output from Azure CLI"));
            trace!("user already logged into azure");
            match response["id"].as_str() {
                Some(subscription_id) => {
                    log::trace!("Already logged into Azure.");
                    subscription_id.to_string()
                }
                None => panic!("No subscription ID found in Azure CLI response."),
            }
        }
        Err(_) => {
            // If not logged in, prompt the user to log in
            log::trace!("Not logged into Azure. Initiating login process...");
            let mut login_cmd = Command::new("az");
            login_cmd.arg("login");
            match exec_os(&mut login_cmd) {
                Ok(_) => {
                    log::trace!("Login to Azure succeeded!");

                    // After login, re-attempt to get the subscription ID
                    let mut post_login_cmd = Command::new("az");
                    post_login_cmd.arg("account").arg("show");
                    match exec_os(&mut post_login_cmd) {
                        Ok(post_output) => {
                            let post_response: Value = serde_json::from_str(&post_output)
                                .unwrap_or_else(|_| {
                                    panic!("Failed to parse JSON output from Azure CLI post-login")
                                });

                            match post_response["id"].as_str() {
                                Some(subscription_id) => subscription_id.to_string(),
                                None => panic!(
                                    "No subscription ID found in Azure CLI response after login."
                                ),
                            }
                        }
                        Err(_) => {
                            panic!("Failed to retrieve subscription ID even after logging in.")
                        }
                    }
                }
                Err(e) => panic!("Error executing Azure CLI login: {}", e),
            }
        }
    }
}

pub fn get_azure_token() -> Result<String, String> {
    let _ = verify_login_or_panic();

    let mut command = Command::new("az");
    command.arg("account").arg("get-access-token");

    match exec_os(&mut command) {
        Ok(output) => {
            let response: Value = serde_json::from_str(&output)
                .map_err(|e| format!("Failed to parse JSON output: {}", e))?;
            response["accessToken"]
                .as_str()
                .map(|s| s.to_string())
                .ok_or_else(|| "Access token not found in Azure CLI response.".to_string())
        }
        Err(e) => Err(e),
    }
}

pub fn create_resource_group(resource_group_name: &str, location: &str) -> Result<(), String> {
    if !is_location_valid(location)? {
        return Err(format!("Invalid location: {}", location));
    }

    if resource_group_exists(resource_group_name)? {
        return Ok(());
    }

    let mut cmd = Command::new("az");
    cmd.arg("group")
        .arg("create")
        .arg("--name")
        .arg(resource_group_name)
        .arg("--location")
        .arg(location);

    match exec_os(&mut cmd) {
        Ok(_) => Ok(()),
        Err(e) => Err(format!(
            "Failed to create resource group {}. Error: {}",
            resource_group_name, e
        )),
    }
}

pub fn resource_group_exists(resource_group_name: &str) -> Result<bool, String> {
    let mut cmd = Command::new("az");

    cmd.arg("group")
        .arg("exists")
        .arg("--name")
        .arg(resource_group_name);

    match exec_os(&mut cmd) {
        Ok(output) => {
            let result_str = output.trim().to_string();
            match result_str.as_str() {
                "true" => Ok(true),
                "false" => Ok(false),
                _ => Err(format!("Unexpected output from Azure CLI: {}", result_str)),
            }
        }
        Err(e) => Err(format!(
            "Failed to check existence of resource group {}. Error: {}",
            resource_group_name, e
        )),
    }
}
pub fn delete_resource_group(resource_group_name: &str) -> Result<(), String> {
    let mut command = Command::new("az");
    command
        .arg("group")
        .arg("delete")
        .arg("--name")
        .arg(resource_group_name)
        .arg("--yes") // Automatically confirm the deletion without prompt.
        .arg("--no-wait"); // Don't wait for the deletion to complete; this makes it non-blocking.

    match exec_os(&mut command) {
        Ok(_) => Ok(()),
        Err(e) => Err(format!(
            "Failed to delete resource group {}. Error: {}",
            resource_group_name, e
        )),
    }
}

lazy_static! {
    static ref LOCATIONS_CACHE: Mutex<Option<HashSet<String>>> = Mutex::new(None);
}

pub fn is_location_valid(location: &str) -> Result<bool, String> {
    let mut cached_locations = LOCATIONS_CACHE.lock().unwrap();

    // Step 1: Check the cache for the location
    if let Some(ref locations) = *cached_locations {
        if locations.contains(location) {
            return Ok(true);
        }
    }

    // Step 2: If not in cache, fetch from Azure
    let mut command = Command::new("az");
    command.args(&[
        "account",
        "list-locations",
        "--query",
        &format!(
            "[?name == '{}' || displayName == '{}'].{{Name: name, DisplayName: displayName}}",
            location, location
        ),
    ]);
    match exec_os(&mut command) {
        Ok(output) => {
            let available_locations: Vec<Value> = serde_json::from_str(&output)
                .map_err(|e| format!("Error parsing locations: {}", e))?;

            // Step 3: Update the cache with fetched locations
            let mut new_locations = HashSet::new();
            for loc in available_locations.iter() {
                if let Some(name) = loc["Name"].as_str() {
                    new_locations.insert(name.to_string());
                }
                if let Some(display_name) = loc["DisplayName"].as_str() {
                    new_locations.insert(display_name.to_string());
                }
            }
            *cached_locations = Some(new_locations);

            if cached_locations.as_ref().unwrap().contains(location) {
                return Ok(true);
            }
        }
        Err(e) => return Err(format!("Error executing Azure CLI: {}", e)),
    }

    Ok(false)
}

pub fn create_cosmos_account(
    resource_group_name: &str,
    db_name: &str,
    location: &str,
) -> Result<(), String> {
    if !is_location_valid(location)? {
        return Err(format!("Invalid location: {}", location));
    }

    if cosmos_account_exists(db_name, resource_group_name)? {
        return Ok(());
    }

    let kind = "GlobalDocumentDB"; // For SQL API. Change if you need a different API.

    let mut command = Command::new("az");
    command
        .arg("cosmosdb")
        .arg("create")
        .arg("--name")
        .arg(db_name)
        .arg("--resource-group")
        .arg(resource_group_name)
        .arg("--kind")
        .arg(kind)
        .arg("--locations")
        .arg(format!("regionName={}", location))
        .arg("--capabilities")
        .arg("EnableServerless");

    match exec_os(&mut command) {
        Ok(output) => {
            log::trace!("stdout: {}", output);
            Ok(())
        }
        Err(error) => Err(error),
    }
}
pub fn delete_cosmos_account(
    cosmos_db_name: &str,
    resource_group_name: &str,
) -> Result<(), String> {
    let mut command = Command::new("az");
    command
        .arg("cosmosdb")
        .arg("delete")
        .arg("--name")
        .arg(cosmos_db_name)
        .arg("--resource-group")
        .arg(resource_group_name)
        .arg("--yes"); // Automatically confirm the deletion without prompt.

    match exec_os(&mut command) {
        Ok(_) => Ok(()),
        Err(error) => Err(error),
    }
}

pub fn cosmos_account_exists(
    cosmos_db_name: &str,
    resource_group_name: &str,
) -> Result<bool, String> {
    let mut command = Command::new("az");
    command
        .arg("cosmosdb")
        .arg("list")
        .arg("--resource-group")
        .arg(resource_group_name)
        .arg("--query")
        .arg(format!("[?name=='{}']", cosmos_db_name))
        .arg("--output")
        .arg("json");

    match exec_os(&mut command) {
        Ok(output) => {
            if output.trim().is_empty() || output.trim() == "[]" {
                Ok(false)
            } else {
                Ok(true)
            }
        }
        Err(error) => Err(error),
    }
}

pub fn get_cosmos_secrets(
    cosmos_db_name: &str,
    resource_group: &str,
) -> Result<Vec<CosmosSecret>, String> {
    let sub_id = verify_login_or_panic();
    let mut command = Command::new("az");
    command
        .arg("cosmosdb")
        .arg("keys")
        .arg("list")
        .arg("--name")
        .arg(cosmos_db_name)
        .arg("--subscription")
        .arg(&sub_id)
        .arg("--type")
        .arg("connection-strings")
        .arg("--resource-group")
        .arg(&resource_group);

    match exec_os(&mut command) {
        Ok(output) => {
            let output: CosmosSecretsOutput = serde_json::from_str(&output)
                .map_err(|e| format!("Failed to parse JSON output: {}", e))?;
            Ok(output.connection_strings)
        }
        Err(error) => Err(error),
    }
}

pub fn store_cosmos_secrets_in_keyvault(
    secrets: &CosmosSecret,
    keyvault_name: &str,
) -> Result<(), String> {
    let secrets_json = serde_json::to_string(secrets)
        .map_err(|e| format!("Failed to serialize CosmosSecrets to JSON: {}", e))?;
    save_secret(keyvault_name, "cosmos-secrets", &secrets_json)
}

pub fn retrieve_cosmos_secrets_from_keyvault(keyvault_name: &str) -> Result<CosmosSecret, String> {
    let cosmos_secret_str = match get_secret(keyvault_name, "cosmos-secrets") {
        Ok(s) => s,
        Err(e) => {
            return Err(format!(
                "Failed to retrieve cosmos-secrets from Key Vault. {}",
                e
            ))
        }
    };
    // Deserialize the cosmos_secret_str into CosmosSecret
    let secrets: CosmosSecret = serde_json::from_str(&cosmos_secret_str)
        .map_err(|e| format!("Error parsing cosmos secret from Key Vault: {}", e))?;

    Ok(secrets)
}
pub fn create_database(
    account_name: &str,
    database_name: &str,
    resource_group: &str,
) -> Result<(), String> {
    if cosmos_database_exists(account_name, database_name, resource_group)? {
        log::trace!("Database {} already exists.", database_name);
        return Ok(());
    }
    let mut command = Command::new("az");
    command
        .arg("cosmosdb")
        .arg("sql")
        .arg("database")
        .arg("create")
        .arg("--account-name")
        .arg(account_name)
        .arg("--name")
        .arg(database_name)
        .arg("--resource-group")
        .arg(resource_group);

    match exec_os(&mut command) {
        Ok(_output) => {
            log::trace!("Created database: {}", database_name);
            Ok(())
        }
        Err(error) => Err(error),
    }
}

pub fn cosmos_database_exists(
    account_name: &str,
    database_name: &str,
    resource_group: &str,
) -> Result<bool, String> {
    let mut command = Command::new("az");
    command
        .arg("cosmosdb")
        .arg("sql")
        .arg("database")
        .arg("show")
        .arg("--account-name")
        .arg(account_name)
        .arg("--name")
        .arg(database_name)
        .arg("--resource-group")
        .arg(resource_group);

    match exec_os(&mut command) {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}
pub fn create_collection(
    account_name: &str,
    database_name: &str,
    collection_name: &str,
    resource_group: &str,
) -> Result<(), String> {
    if cosmos_collection_exists(account_name, database_name, collection_name, resource_group)? {
        log::trace!("Collection {} already exists", collection_name);
        return Ok(());
    }

    // Construct the command without executing it.
    let mut command = Command::new("az");
    command
        .arg("cosmosdb")
        .arg("sql")
        .arg("container")
        .arg("create")
        .arg("--account-name")
        .arg(account_name)
        .arg("--database-name")
        .arg(database_name)
        .arg("--name")
        .arg(collection_name)
        .arg("--resource-group")
        .arg(resource_group)
        .arg("--partition-key-path")
        .arg("/partitionKey");

    // Use exec_os to execute the command
    match exec_os(&mut command) {
        Ok(output) => {
            log::trace!("Output: {}", output);
            Ok(())
        }
        Err(error) => {
            Err(format!(
                "Failed to create Cosmos SQL container {} in database {} for account {} in resource group {}: {}",
                collection_name, database_name, account_name, resource_group, error
            ))
        }
    }
}

pub fn cosmos_collection_exists(
    account_name: &str,
    database_name: &str,
    collection_name: &str,
    resource_group: &str,
) -> Result<bool, String> {
    let mut command = Command::new("az");
    command
        .arg("cosmosdb")
        .arg("sql")
        .arg("container")
        .arg("show")
        .arg("--account-name")
        .arg(account_name)
        .arg("--database-name")
        .arg(database_name)
        .arg("--name")
        .arg(collection_name)
        .arg("--resource-group")
        .arg(resource_group);

    match exec_os(&mut command) {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}

lazy_static! {
    static ref ENV_MAP: HashMap<String, String> = {
        let mut m = HashMap::new();
        for (key, value) in std::env::vars() {
            m.insert(value, key);
        }
        m
    };
}

fn get_env_name(value: &str) -> Option<String> {
    ENV_MAP.get(value).cloned()
}
pub fn print_cmd(command: &Command) {
    let program = command.get_program().to_string_lossy();
    let args = command.get_args();
    let re = Regex::new(r"(AccountKey=)([^;]+)").unwrap();
    let mut cmd_str: Vec<String> = std::iter::once(program.into_owned())
        .chain(args.map(|arg| arg.to_string_lossy().into_owned()))
        .collect();

    // Hide environment variable values using ENV_MAP
    for arg in &mut cmd_str {
        // Replace argument values that are environment variable values with their variable names
        if let Some(env_name) = ENV_MAP.get(arg) {
            *arg = format!("${}", env_name);
        } else {
            // If not an environment variable value, check for AccountKey and mask it
            *arg = re.replace(arg, |caps: &regex::Captures| {
                let key_length = caps[2].len();
                format!("AccountKey={}x==", "X".repeat(key_length - 3))
            }).to_string();
        }
    }

    info!("Executing: {}", cmd_str.join(" "));
}

pub fn keyvault_exists(kv_name: &str) -> Result<bool, String> {
    let mut command = Command::new("az");
    command
        .arg("keyvault")
        .arg("show")
        .arg("--name")
        .arg(kv_name);

    let output = exec_os(&mut command)?;

    if output.contains(kv_name) {
        log::trace!("KV {} already exists", kv_name);
        Ok(true)
    } else {
        log::trace!("{} does not exist", kv_name);
        Ok(false)
    }
}

pub fn save_secret(
    keyvault_name: &str,
    secret_name: &str,
    secret_value: &str,
) -> Result<(), String> {
    let mut command = Command::new("az");
    command
        .arg("keyvault")
        .arg("secret")
        .arg("set")
        .arg("--vault-name")
        .arg(keyvault_name)
        .arg("--name")
        .arg(secret_name)
        .arg("--value")
        .arg(secret_value);

    exec_os(&mut command).map(|_| ())
}

pub fn get_secret(keyvault_name: &str, secret_name: &str) -> Result<String, String> {
    let mut command = Command::new("az");
    command
        .arg("keyvault")
        .arg("secret")
        .arg("show")
        .arg("--vault-name")
        .arg(keyvault_name)
        .arg("--name")
        .arg(secret_name);

    match exec_os(&mut command) {
        Ok(secret_json) => {
            // Parse the top-level JSON
            let top_level: serde_json::Value = serde_json::from_str(&secret_json)
                .map_err(|e| format!("Error parsing Key Vault response: {}", e))?;

            // Extract the value field as String
            top_level["value"]
                .as_str()
                .map(|s| s.to_string())
                .ok_or(format!(
                    "Missing 'value' field in Key Vault response for {}.",
                    secret_name
                ))
        }
        Err(e) => Err(e),
    }
}

fn exec_os(command: &mut Command) -> Result<String, String> {
    print_cmd(command);

    let output = command
        .output()
        .map_err(|e| format!("Error executing command: {}", e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).into_owned())
    }
}
///
///  az communication sms send --sender +1866XXXYYYY  --recipient +1206XXXYYYY --message "Hey -- this is a test!"
///  sender must be configured in the communication service and must be a toll free number
///  requires AZURE_COMMUNICATION_CONNECTION_STRING to be set as an environment variable
pub fn send_text_message(to: &str, msg: &str) -> Result<(), String> {
    let mut command = Command::new("az");
    command
        .arg("communication")
        .arg("sms")
        .arg("send")
        .arg("--sender")
        .arg(&CATAN_ENV.service_phone_number)
        .arg("--recipient")
        .arg(to)
        .arg("--message")
        .arg(msg);

    // Use exec_os to execute the command
    match exec_os(&mut command) {
        Ok(output) => {
            log::trace!("Output: {}", output);
            Ok(())
        }
        Err(error) => Err(format!("Failed to send message. Error: {:#?}", error)),
    }
}

///
/// az communication email send --sender "<provisioned email>" --subject "Test email" --to "xxxx@outlook.com"  --text "This is a test from the Catan Service"
/// the sender email must be provisioned in Azure
/// requires AZURE_COMMUNICATION_CONNECTION_STRING to be set as an environment variable

pub fn send_email(to: &str, from: &str, subject: &str, msg: &str) -> Result<(), String> {
    trace_function!("azure_wrappers::send_email");
    let mut command = Command::new("az");
    command
        .arg("communication")
        .arg("email")
        .arg("send")
        .arg("--sender")
        .arg(from)
        .arg("--to")
        .arg(to)
        .arg("--subject")
        .arg(subject)
        .arg("--text")
        .arg(msg);

    // Use exec_os to execute the command
    match exec_os(&mut command) {
        Ok(output) => {
            log::trace!("Output: {}", output);
            Ok(())
        }
        Err(error) => Err(format!("Failed to send message. Error: {:#?}", error)),
    }
}

#[test]
pub fn send_text_message_test() {
    send_text_message(&CATAN_ENV.test_phone_number, "this is a test")
        .expect("text message should be sent");
}

#[test]
pub fn send_email_test() {
    //
    //  run the async function synchronously
    let runtime = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
    runtime.block_on(init_env_logger(
        log::LevelFilter::Trace,
        log::LevelFilter::Error,
    ));
    send_email(&CATAN_ENV.test_email, &CATAN_ENV.service_email, "this is a test", "test email")
        .expect("text message should be sent");
}

#[test]
pub fn azure_resources_integration_test() {
    let three_letters = "abc";
    let resource_group = "test-resource-group-".to_owned() + three_letters;
    let location = &CATAN_ENV.azure_location;
    let kv_name = std::env::var("KEV_VAULT_NAME").expect("KEV_VAULT_NAME not found in environment");
    let cosmos_account_name = "test-cosmos-account-".to_owned() + three_letters;
    let database_name = "test-cosmos-database-".to_owned() + three_letters;
    let collection_name = "test-collection-".to_owned() + three_letters;

    //
    //  run the async function synchronously
    let runtime = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
    runtime.block_on(init_env_logger(
        log::LevelFilter::Trace,
        log::LevelFilter::Error,
    ));

    // make sure the user is logged in

    verify_login_or_panic();

    // verify KV exists
    keyvault_exists(&kv_name).expect(&format!("Failed to find Key Vault named {}.", kv_name));

    // Create a test resource group
    log::info!("creating resource group");
    create_resource_group(&resource_group, location).expect("Failed to create resource group.");

    log::trace!("creating cosmosdb: {}", cosmos_account_name);
    //Add a Cosmos DB instance to it
    create_cosmos_account(&resource_group, &cosmos_account_name, location)
        .expect("Failed to create Cosmos DB instance.");

    log::trace!("Creating database: {}", database_name);
    create_database(&cosmos_account_name, &database_name, &resource_group)
        .expect("creating a cosmos db should succeed");
    // Create a collection in the Cosmos DB instance
    log::trace!("Creating collection: {}", collection_name);
    create_collection(
        &cosmos_account_name,
        &database_name,
        &collection_name,
        &resource_group,
    )
    .expect("Failed to create collection in Cosmos DB.");

    //get cosmos secrets from cosmos
    let secrets = get_cosmos_secrets(&cosmos_account_name, &resource_group)
        .expect("Failed to retrieve Cosmos DB secrets.");

    // find the secondary read/write secret
    let secret = secrets
        .iter()
        .find(|s| s.key_kind == "Secondary")
        .expect("there should be a Secondary key in the cosmos secrets");

    // store in key vault
    store_cosmos_secrets_in_keyvault(&secret, &kv_name)
        .expect("Failed to store secrets in Key Vault.");

    //Validate that the resource group, Cosmos DB, and Key Vault exist
    assert!(
        resource_group_exists(&resource_group).expect("Failed to check resource group existence.")
    );
    assert!(cosmos_account_exists(&cosmos_account_name, &resource_group)
        .expect("Failed to check Cosmos DB existence."));

    //  Get the secrets back out of Key Vault and validate they are correct
    let retrieved_secrets = retrieve_cosmos_secrets_from_keyvault(&kv_name)
        .expect("Failed to retrieve secrets from Key Vault.");
    assert_eq!(
        *secret, retrieved_secrets,
        "Stored and retrieved secrets do not match."
    );

    // Delete Cosmos DB - commented out as this take *forever* to run
    // delete_cosmos_account(&cosmos_account_name, &resource_group)
    //     .expect("Failed to delete Cosmos DB.");

    // Clean up: Delete the test resource group (you can comment this out if you want to inspect resources)
    delete_resource_group(&resource_group).expect("Failed to delete resource group.");
}

#[test]
pub fn rotate_login_keys() {
    let kv_name = std::env::var("KEV_VAULT_NAME").expect("KEY_VAULT_NAME not found in environment");
    let old_name = "oldLoginSecret-test";
    let new_name = "currentLoginSecret-test";

    // make sure the user is logged in
    verify_login_or_panic();

    // verify KV exists
    keyvault_exists(&kv_name).expect(&format!("Failed to find Key Vault named {}.", kv_name));

    let current_login_key = get_secret(&kv_name, new_name).unwrap_or_else(|_| generate_jwt_key());
    let test_context = TestContext::new(false);
    let claims = Claims::new(
        "test_id",
        "test@email.com",
        24 * 60 * 60,
        &Some(test_context),
    );
    let token =
        create_jwt_token(&claims, &current_login_key).expect("create token should not fail");

    let new_key = generate_jwt_key();

    let _ = save_secret(&kv_name, old_name, &current_login_key);
    let _ = save_secret(&kv_name, new_name, &new_key);

    let current_login_key_again =
        get_secret(&kv_name, new_name).unwrap_or_else(|_| generate_jwt_key());

    let res_current = validate_jwt_token(&token, &current_login_key_again);
    assert!(res_current.is_none());

    let old_key = get_secret(&kv_name, old_name).unwrap_or_else(|_| generate_jwt_key());

    let res_old = validate_jwt_token(&token, &old_key);
    assert!(res_old.is_some());
}
