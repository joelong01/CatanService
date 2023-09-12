#![allow(dead_code)]
#![allow(unused_imports)]
use log::trace;
use once_cell::sync::Lazy;
use once_cell::sync::OnceCell;
use rand::Rng;
use regex::Regex;
use reqwest::StatusCode;
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

use crate::az_error_to_service_response;
use crate::bad_request_from_string;
use crate::init_env_logger;
use crate::log_and_return_azure_core_error;
use crate::log_return_bad_request;
use crate::log_return_serde_error;
use crate::middleware::request_context_mw::TestContext;
use crate::middleware::request_context_mw::SERVICE_CONFIG;
use crate::shared::service_models::Claims;
use crate::shared::service_models::Role;
use crate::shared::shared_models::GameError;
use crate::shared::shared_models::ResponseType;
use crate::shared::shared_models::ServiceResponse;
use crate::trace_function;
use crate::user_service::users::create_jwt_token;
use crate::user_service::users::generate_jwt_key;
use crate::user_service::users::validate_jwt_token;

use super::azure_types::CosmosDatabaseInfo;

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

/// Verifies if the user is already logged into Azure.
///
/// If the user is logged in, the subscription ID is returned.
/// If not logged in, the function prompts the user to log in.
/// In the case where the user is unable to login or if the subscription ID
/// cannot be retrieved, the function will panic.
///
/// Returns:
///   - A `String` containing the Azure subscription ID.
///
pub fn verify_login_or_panic() -> String {
    match verify_login() {
        Ok(id) => id,
        Err(service_response) => panic!(
            "Need to login to Azure for this service to work: {}",
            service_response
        ),
    }
}

fn verify_login() -> Result<String, ServiceResponse> {
    // Check if the user is already logged into Azure
    if let Some(subscription_id) = SUBSCRIPTION_ID.get() {
        trace!("user already logged into azure");
        return Ok(subscription_id.clone());
    }

    let args = ["account", "show"];
    print_cmd(&args);
    let output = exec_os(&args)?;
    let response: Value = serde_json::from_str(&output)?;

    trace!("user already logged into azure");
    if let Some(subscription_id) = response["id"].as_str() {
        log::trace!("Already logged into Azure.");
        return Ok(subscription_id.to_string());
    } else {
        // If not logged in, prompt the user to log in
        log::trace!("Not logged into Azure. Initiating login process...");
        let args = ["login"];
        print_cmd(&args);
        exec_os(&args)?;

        log::trace!("Login to Azure succeeded!");

        // After login, re-attempt to get the subscription ID
        let args = &["account", "show"];
        print_cmd(args);
        let post_output = exec_os(args)?;
        let post_response: Value = serde_json::from_str(&post_output)?;

        if let Some(subscription_id) = post_response["id"].as_str() {
            Ok(subscription_id.to_string())
        } else {
            Err(ServiceResponse {
                message: "No subscription ID found in Azure CLI response after login.".to_string(),
                status: StatusCode::INTERNAL_SERVER_ERROR,
                response_type: ResponseType::NoData,
                game_error: GameError::AzError(String::default()),
            })
        }
    }
}

/// Retrieves an access token from Azure.
///
/// The function assumes that the user is already logged into Azure.
/// If not logged in, the function will prompt the user to log in
/// (this behavior is due to the call to `verify_login_or_panic`).
///
/// Returns:
///   - `Ok(String)`: The Azure access token.
///   - `Err(ServiceResponse)`: An error indicating the reason for the failure.
pub fn get_azure_token() -> Result<String, ServiceResponse> {
    let _ = verify_login_or_panic();
    let args = ["account", "get-access-token"];
    let cmd = print_cmd(&args);

    let output = exec_os(&args)?;

    // this should work, but the compiler is having trouble resolving the "From chain" serder_error -> GameError
    // but for whatever reason can go from GameError to ServiceError
    // let json: Value = serde_json::from_str(&output)?;
    let json: Value = serde_json::from_str(&output).map_err(ServiceResponse::from)?;

    match json["accessToken"].as_str() {
        Some(v) => Ok(v.to_string()),
        None => {
            az_error_to_service_response!(
                cmd,
                "Access token not found in Azure CLI response.".to_string()
            )
        }
    }
}

/// Creates a new Azure resource group.
///
/// This function creates a new resource group in Azure in the given location.
/// If the location is invalid or the resource group already exists, it returns an error.
///
/// Parameters:
///   - `resource_group_name`: The name of the resource group to be created.
///   - `location`: The Azure location where the resource group should be created.
///
/// Returns:
///   - `Ok(())`: If the resource group creation is successful.
///   - `ServiceResponse`: An error message describing the reason for the failure.
pub fn create_resource_group(
    resource_group_name: &str,
    location: &str,
) -> Result<(), ServiceResponse> {
    is_location_valid(location)?;

    if resource_group_exists(resource_group_name)? {
        return Ok(());
    }

    let cmd_args = [
        "group",
        "create",
        "--name",
        resource_group_name,
        "--location",
        location,
    ];
    print_cmd(&cmd_args);
    exec_os(&cmd_args)?;
    Ok(())
}
/// Checks if an Azure resource group exists.
///
/// This function checks if a resource group with the given name exists in Azure.
///
/// Parameters:
///   - `resource_group_name`: The name of the resource group to check.
///
/// Returns:
///   - `Ok(bool)`: `true` if the resource group exists, `false` otherwise.
///   - `ServiceResponse`: An error message describing the reason for the failure.
pub fn resource_group_exists(resource_group_name: &str) -> Result<bool, ServiceResponse> {
    let cmd_args = ["group", "exists", "--name", resource_group_name];
    let cmd = print_cmd(&cmd_args);
    let output = exec_os(&cmd_args)?;

    let result_str = output.trim().to_string();
    match result_str.as_str() {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => az_error_to_service_response!(
            &cmd,
            format!("Unexpected output from Azure CLI: {}", result_str)
        ),
    }
}

/// Deletes an Azure resource group.
///
/// This function attempts to delete a resource group with the given name from Azure.
///
/// Parameters:
///   - `resource_group_name`: The name of the resource group to be deleted.
///
/// Returns:
///   - `Ok(())`: If the resource group deletion is successful.
///   - `ServiceResponse`: An error message describing the reason for the failure.
pub fn delete_resource_group(resource_group_name: &str) -> Result<(), ServiceResponse> {
    let cmd_args = [
        "group",
        "delete",
        "--name",
        resource_group_name,
        "--yes",
        "--no-wait",
    ];
    print_cmd(&cmd_args);
    exec_os(&cmd_args)?;
    Ok(())
}

lazy_static! {
    static ref LOCATIONS_CACHE: Mutex<Option<HashSet<String>>> = Mutex::new(None);
}

/// Checks if a location is valid within Azure.
///
/// This function checks if a given location is valid by first checking a local cache,
/// and then if necessary, querying Azure directly.
///
/// Parameters:
///   - `location`: The name or display name of the location to check.
///
/// Returns:
///   - `Ok(bool)`: `true` if the location is valid, `false` otherwise.
///   - `Err(ServiceResponse)`: An error describing the reason for the failure.
pub fn is_location_valid(location: &str) -> Result<bool, ServiceResponse> {
    let mut cached_locations = LOCATIONS_CACHE.lock().unwrap();

    // Step 1: Check the cache for the location
    if let Some(ref locations) = *cached_locations {
        if locations.contains(location) {
            return Ok(true);
        }
    }

    // Step 2: If not in cache, fetch from Azure
    let cmd_args = [
        "account",
        "list-locations",
        "--query",
        &format!(
            "[?name == '{}' || displayName == '{}'].{{Name: name, DisplayName: displayName}}",
            location, location
        ),
    ];
    print_cmd(&cmd_args);
    let output = exec_os(&cmd_args)?;

    let available_locations =
        serde_json::from_str::<Vec<Value>>(&output).map_err(ServiceResponse::from)?;

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

    Ok(false)
}
/// Executes the 'az' command with the provided arguments.
///
/// This function invokes the Azure CLI ('az') with the given arguments. It captures the stdout
/// and stderr of the call and if az iteself cannot be executed, it returns an error.  otherwise
/// it returns what was in stdout or stderr - this is like calling a REST api that does a database lookup
/// if the id is not in the db, you return a 200 with a "not found" success message vs. a 404
///
/// Parameters:
///   - `args`: A slice containing the command arguments for the 'az' CLI.
///
/// Returns:
///   - `Ok(String)`: The stdout *or* stderr of the command if it executes successfully.
///   - `Err(ServiceResponse)`: A ServiceResponse error capturing the reason for the failure to exec az
///
/// Example:
/// ```
/// let result = exec_os(&["account", "list"]);
/// ```
fn exec_os(args: &[&str]) -> Result<String, ServiceResponse> {
    let mut command = Command::new("az");
    command.args(args);

    let output = command
        .output()
        .map_err(|err| ServiceResponse{
            message: format!("Failed to execute command: {:?}", err),
            status: StatusCode::BAD_REQUEST,
            response_type: ResponseType::AzError(err.to_string()),
            game_error: GameError::AzError(err.to_string()),
        })?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        let error_msg = String::from_utf8_lossy(&output.stderr).into_owned();
        Ok(error_msg)
    }
}

/// Creates a Cosmos DB account.
///
/// This function creates a Cosmos DB account in a given location and resource group if it doesn't exist.
///
/// Parameters:
///   - `resource_group_name`: Name of the resource group.
///   - `account_name`: Name of the CosmosDb account.
///   - `location`: Location to create the Cosmos DB in.
///
/// Returns:
///   - `Ok(())`: If the Cosmos DB account was successfully created.
///   - `Err(ServiceResponse)`: An ServiceResponse message describing the reason for the failure.
pub fn create_cosmos_account(
    resource_group_name: &str,
    account_name: &str,
    location: &str,
) -> Result<(), ServiceResponse> {
    if !is_location_valid(location)? {
        return Err(bad_request_from_string!(&format!(
            "Invalid location: {}",
            location
        )));
    }

    if cosmos_account_exists(account_name, resource_group_name)? {
        return Ok(());
    }

    let kind = "GlobalDocumentDB"; // For SQL API. Change if you need a different API.

    let cmd_args = [
        "cosmosdb",
        "create",
        "--name",
        account_name,
        "--resource-group",
        resource_group_name,
        "--kind",
        kind,
        "--locations",
        &format!("regionName={}", location),
        "--capabilities",
        "EnableServerless",
    ];
    print_cmd(&cmd_args);
    let output = exec_os(&cmd_args)?;

    log::trace!("stdout: {}", output);
    Ok(())
}

/// Deletes a Cosmos DB account.
///
/// Parameters:
///   - `cosmos_db_name`: Name of the Cosmos DB.
///   - `resource_group_name`: Name of the resource group.
///
/// Returns:
///   - `Ok(())`: If the Cosmos DB account was successfully deleted.
///   - `Err(ServiceResponse)`: An error message describing the reason for the failure.
pub fn delete_cosmos_account(
    cosmos_db_name: &str,
    resource_group_name: &str,
) -> Result<(), ServiceResponse> {
    let cmd_args = [
        "cosmosdb",
        "delete",
        "--name",
        cosmos_db_name,
        "--resource-group",
        resource_group_name,
        "--yes", // Automatically confirm the deletion without prompt.
    ];
    print_cmd(&cmd_args);
    exec_os(&cmd_args)?;
    Ok(())
}

/// Checks if a Cosmos DB account exists.
///
/// Parameters:
///   - `cosmos_db_name`: Name of the Cosmos DB.
///   - `resource_group_name`: Name of the resource group.
///
/// Returns:
///   - `Ok(bool)`: `true` if the Cosmos DB account exists, `false` otherwise.
///   - `Err(ServiceResponse)`: An error message describing the reason for the failure.
pub fn cosmos_account_exists(
    cosmos_db_name: &str,
    resource_group_name: &str,
) -> Result<bool, ServiceResponse> {
    let cmd_args = [
        "cosmosdb",
        "list",
        "--resource-group",
        resource_group_name,
        "--query",
        &format!("[?name=='{}']", cosmos_db_name),
        "--output",
        "json",
    ];
    print_cmd(&cmd_args);
    let json_doc = exec_os(&cmd_args)?;
    match serde_json::from_str::<Vec<CosmosDatabaseInfo>>(&json_doc) {
        Ok(accounts) => {
            if accounts.len() > 0 {
                return Ok(true);
            }
            return Ok(false);
        }
        Err(_) => Ok(false),
    }
}
/// Retrieves secrets for a given Cosmos DB.
///
/// # Parameters:
/// - `cosmos_db_name`: Name of the Cosmos DB.
/// - `resource_group`: Name of the resource group where the Cosmos DB resides.
///
/// # Returns:
/// - `Result<Vec<CosmosSecret>, String>`: On success, returns a vector of Cosmos secrets. On failure, returns an error message.
/// - `Err(ServiceResponse)`: An error message describing the reason for the failure.
pub fn get_cosmos_secrets(
    cosmos_db_name: &str,
    resource_group: &str,
) -> Result<Vec<CosmosSecret>, ServiceResponse> {
    let sub_id = verify_login_or_panic();
    let cmd_args = [
        "cosmosdb",
        "keys",
        "list",
        "--name",
        cosmos_db_name,
        "--subscription",
        &sub_id,
        "--type",
        "connection-strings",
        "--resource-group",
        &resource_group,
    ];
    print_cmd(&cmd_args);
    let output = exec_os(&cmd_args)?;
    match serde_json::from_str::<CosmosSecretsOutput>(&output) {
        Ok(secrets) => Ok(secrets.connection_strings),
        Err(_) => Ok(Vec::new()),
    }
}

/// Stores Cosmos DB secrets in a given Azure Key Vault.
///
/// # Parameters:
/// - `secrets`: The Cosmos DB secrets.
/// - `keyvault_name`: Name of the Azure Key Vault.
///
/// # Returns:
/// - `Result<(), String>`: On success, returns unit type. On failure, returns an error message.
/// - `Err(ServiceResponse)`: An error message describing the reason for the failure.
pub fn store_cosmos_secrets_in_keyvault(
    secrets: &CosmosSecret,
    keyvault_name: &str,
) -> Result<(), ServiceResponse> {
    let secrets_json = serde_json::to_string(secrets).map_err(ServiceResponse::from)?;
    save_secret(keyvault_name, "cosmos-secrets", &secrets_json)?;
    Ok(())
}

/// Retrieves Cosmos DB secrets from a given Azure Key Vault.
///
/// # Parameters:
/// - `keyvault_name`: Name of the Azure Key Vault.
///
/// # Returns:
/// - `Result<CosmosSecret, String>`: On success, returns the Cosmos secrets. On failure, returns an error message.
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

/// Creates a Cosmos SQL database if it does not exist.
///
/// # Parameters:
/// - `account_name`: Name of the Cosmos account.
/// - `database_name`: Name of the database to create.
/// - `resource_group`: Name of the resource group.
///
/// # Returns:
/// - `Result<(), ServiceResponse>`: On success, returns unit type. On failure, returns ServiceResponse.
pub fn create_database(
    account_name: &str,
    database_name: &str,
    resource_group: &str,
) -> Result<(), ServiceResponse> {
    if cosmos_database_exists(account_name, database_name, resource_group)? {
        log::trace!("Database {} already exists.", database_name);
        return Ok(());
    }

    let cmd_args = [
        "cosmosdb",
        "sql",
        "database",
        "create",
        "--account-name",
        account_name,
        "--name",
        database_name,
        "--resource-group",
        resource_group,
    ];
    print_cmd(&cmd_args);
    let _ = exec_os(&cmd_args)?;
    Ok(())
}

/// Checks if a Cosmos SQL database exists.
///
/// # Parameters:
/// - `account_name`: Name of the Cosmos account.
/// - `database_name`: Name of the database to check.
/// - `resource_group`: Name of the resource group.
///
/// # Returns:
/// - `Result<bool, String>`: On success, returns whether the database exists or not. On failure, returns an ServiceResponse message.
pub fn cosmos_database_exists(
    account_name: &str,
    database_name: &str,
    resource_group: &str,
) -> Result<bool, ServiceResponse> {
    let cmd_args = [
        "cosmosdb",
        "sql",
        "database",
        "show",
        "--account-name",
        account_name,
        "--name",
        database_name,
        "--resource-group",
        resource_group,
    ];
    print_cmd(&cmd_args);
    let json_doc = exec_os(&cmd_args)?;
    if serde_json::from_str::<CosmosDatabaseInfo>(&json_doc).is_ok() {
        return Ok(true);
    }
    Ok(false)
}

/// Creates a Cosmos SQL collection if it does not exist.
///
/// # Parameters:
/// - `account_name`: Name of the Cosmos account.
/// - `database_name`: Name of the database.
/// - `collection_name`: Name of the collection to create.
/// - `resource_group`: Name of the resource group.
///
/// # Returns:
/// - `Result<(), ServiceResponse>`: On success, returns unit type. On failure, returns an error message.
pub fn create_collection(
    account_name: &str,
    database_name: &str,
    collection_name: &str,
    resource_group: &str,
) -> Result<(), ServiceResponse> {
    if cosmos_collection_exists(account_name, database_name, collection_name, resource_group)? {
        log::trace!("Collection {} already exists", collection_name);
        return Ok(());
    }

    let cmd_args = [
        "cosmosdb",
        "sql",
        "container",
        "create",
        "--account-name",
        account_name,
        "--database-name",
        database_name,
        "--name",
        collection_name,
        "--resource-group",
        resource_group,
        "--partition-key-path",
        "/partitionKey",
    ];
    print_cmd(&cmd_args);
    let output = exec_os(&cmd_args)?;
    log::trace!("Output: {}", output);
    Ok(())
}
/// Checks if a Cosmos SQL collection exists.
///
/// # Parameters:
/// - `account_name`: Name of the Cosmos account.
/// - `database_name`: Name of the database.
/// - `collection_name`: Name of the collection to check.
/// - `resource_group`: Name of the resource group.
///
/// # Returns:
/// - `Result<bool, ServiceResponse>`: On success, returns whether the collection exists or not. On failure, returns an error message.
pub fn cosmos_collection_exists(
    account_name: &str,
    database_name: &str,
    collection_name: &str,
    resource_group: &str,
) -> Result<bool, ServiceResponse> {
    let cmd_args = [
        "cosmosdb",
        "sql",
        "container",
        "show",
        "--account-name",
        account_name,
        "--database-name",
        database_name,
        "--name",
        collection_name,
        "--resource-group",
        resource_group,
    ];
    print_cmd(&cmd_args);
    let json_doc = exec_os(&cmd_args)?;
    if serde_json::from_str::<CosmosDatabaseInfo>(&json_doc).is_ok() {
        return Ok(true);
    }
    Ok(false)
}

pub fn print_cmd(args: &[&str]) -> String {
    let re = Regex::new(r"(AccountKey=)([^;]+)").unwrap();
    let program = "az";
    let mut cmd_str: Vec<String> = std::iter::once(program.to_string())
        .chain(args.iter().map(|&arg| arg.to_string()))
        .collect();

    // Hide environment variable values using ENV_MAP
    let mut i = 0;
    while i < cmd_str.len() {
        // Special handling for `--query`
        if cmd_str[i] == "--query" && i + 1 < cmd_str.len() {
            cmd_str[i + 1] = format!("\"{}\"", cmd_str[i + 1]);
            i += 1; // jump to the --query argument value
        }

        // Replace environment variable values with their variable names
        let arg = &mut cmd_str[i];
        for (value, env_name) in SERVICE_CONFIG.name_value_map.iter() {
            // This will replace all occurrences of the value with the corresponding environment variable name
            *arg = arg.replace(value, &format!("{}", env_name));
        }

        // If not an environment variable value, check for AccountKey and mask it
        *arg = re
            .replace(arg, |caps: &regex::Captures| {
                let key_length = caps[2].len();
                format!("AccountKey={}x==", "X".repeat(key_length - 3))
            })
            .to_string();

        i += 1;
    }

    let cmd = cmd_str.join(" ");
    info!("Executing: {}", cmd);
    cmd
}

/// Checks if a given Azure Key Vault exists.
///
/// # Arguments
///
/// * `kv_name` - The name of the Azure Key Vault.
///
/// # Returns
///
/// * `Ok(true)` if the Key Vault exists, `Ok(false)` otherwise.
pub fn keyvault_exists(kv_name: &str) -> Result<bool, ServiceResponse> {
    let args = ["keyvault", "show", "--name", kv_name];
    print_cmd(&args);
    let output = exec_os(&args)?;

    // Check if the output contains the name of the Key Vault.
    if output.contains(kv_name) {
        log::trace!("KV {} already exists", kv_name);
        Ok(true)
    } else {
        log::trace!("{} does not exist", kv_name);
        Ok(false)
    }
}

/// Saves a secret in an Azure Key Vault.
///
/// # Arguments
///
/// * `keyvault_name` - The name of the Azure Key Vault.
/// * `secret_name` - The name of the secret.
/// * `secret_value` - The value of the secret.
///
/// # Returns
///
/// * `Ok(())` if the secret is saved successfully.
/// - `Err(ServiceResponse)`: An error message describing the reason for the failure.
pub fn save_secret(
    keyvault_name: &str,
    secret_name: &str,
    secret_value: &str,
) -> Result<(), ServiceResponse> {
    let args = [
        "keyvault",
        "secret",
        "set",
        "--vault-name",
        keyvault_name,
        "--name",
        secret_name,
        "--value",
        secret_value,
    ];

    // If the secret_value is longer than 4 characters, modify it for logging
    let displayed_secret_value = if secret_value.len() > 4 {
        format!(
            "{}...{}",
            &secret_value[..2],
            &secret_value[secret_value.len() - 2..]
        )
    } else {
        "<secret>".to_string()
    };

    // Adjusted args for print_cmd
    let print_args = [
        "keyvault",
        "secret",
        "set",
        "--vault-name",
        keyvault_name,
        "--name",
        secret_name,
        "--value",
        &displayed_secret_value,
    ];

    print_cmd(&print_args);

    exec_os(&args)?;
    Ok(())
}

/// Retrieves a secret from an Azure Key Vault.
///
/// # Arguments
///
/// * `keyvault_name` - The name of the Azure Key Vault.
/// * `secret_name` - The name of the secret to retrieve.
///
/// # Returns
///
/// * The value of the retrieved secret wrapped in `Ok` if successful.
/// - `Err(ServiceResponse)`: An error message describing the reason for the failure.
pub fn get_secret(keyvault_name: &str, secret_name: &str) -> Result<String, ServiceResponse> {
    let args = [
        "keyvault",
        "secret",
        "show",
        "--vault-name",
        keyvault_name,
        "--name",
        secret_name,
    ];
    print_cmd(&args);
    let secret_json = exec_os(&args)?;

    // Parse the top-level JSON
    let top_level: serde_json::Value =
        serde_json::from_str(&secret_json).map_err(ServiceResponse::from)?;

    // Extract the 'value' field from the JSON.
    let answer: Result<String, ServiceResponse> = top_level["value"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or(bad_request_from_string!(&format!(
            "secret name not found: {}",
            secret_name
        )));

    answer
}

/// Sends a text message using Azure Communication Services.
///
/// The sender phone number is fetched from SERVICE_CONFIG's `service_phone_number`.
/// It's required to have AZURE_COMMUNICATION_CONNECTION_STRING set as an environment variable.
///
/// # Arguments
///
/// * `to` - The recipient's phone number.
/// * `msg` - The message content.
///
/// # Returns
///
/// * `Ok(())` if the message is sent successfully.
///
/// # Example
///
/// ```ignore
/// az communication sms send --sender +1866XXXYYYY --recipient +1206XXXYYYY --message "Hey -- this is a test!"
/// ```
pub fn send_text_message(to: &str, msg: &str) -> Result<ServiceResponse, ServiceResponse> {
    let args = [
        "communication",
        "sms",
        "send",
        "--sender",
        &SERVICE_CONFIG.service_phone_number,
        "--recipient",
        to,
        "--message",
        msg,
    ];
    print_cmd(&args);
    exec_os(&args)?;
    Ok(ServiceResponse::new(
        "sent",
        StatusCode::OK,
        ResponseType::NoData,
        GameError::NoError(String::default()),
    ))
}

/// Sends an email using Azure Communication Services.
///
/// It's required to have AZURE_COMMUNICATION_CONNECTION_STRING set as an environment variable.
///
/// # Arguments
///
/// * `to` - The recipient's email address.
/// * `from` - The sender's email address (must be provisioned in Azure).
/// * `subject` - The subject of the email.
/// * `msg` - The email message content.
///
/// # Returns
///
/// * `Ok(())` if the email is sent successfully.
///
/// # Example
///
/// ```ignore
/// az communication email send --sender "<provisioned email>" --subject "Test email" --to "xxxx@outlook.com" --text "This is a test from the Catan Service"
/// ```
pub fn send_email(to: &str, from: &str, subject: &str, msg: &str) -> Result<(), String> {
    let args = [
        "communication",
        "email",
        "send",
        "--sender",
        from,
        "--to",
        to,
        "--subject",
        subject,
        "--text",
        msg,
    ];

    // If the msg is longer than 6 characters, modify it for logging
    let displayed_msg = if msg.len() > 6 {
        format!("{}...{}", &msg[..3], &msg[msg.len() - 3..])
    } else {
        msg.to_string()
    };

    // Adjusted args for print_cmd
    let print_args = [
        "communication",
        "email",
        "send",
        "--sender",
        from,
        "--to",
        to,
        "--subject",
        subject,
        "--text",
        &displayed_msg,
    ];

    print_cmd(&print_args);

    match exec_os(&args) {
        Ok(output) => {
            log::trace!("Output: {}", output);
            Ok(())
        }
        Err(error) => Err(format!("Failed to send email. Error: {:#?}", error)),
    }
}

pub fn verify_or_create_account(
    resource_group: &str,
    account_name: &str,
    location: &str,
) -> Result<(), ServiceResponse> {
    let exists = cosmos_account_exists(&account_name, resource_group)?;
    if !exists {
        create_cosmos_account(resource_group, account_name, location)?;
    }
    Ok(())
}

pub fn verify_or_create_database(
    account_name: &str,
    database_name: &str,
    resource_group: &str,
) -> Result<(), ServiceResponse> {
    let exists = cosmos_database_exists(account_name, database_name, resource_group)?;

    if !exists {
        create_database(account_name, database_name, resource_group)?;
    }

    Ok(())
}

pub fn verify_or_create_collection(
    account_name: &str,
    database_name: &str,
    collection_name: &str,
    resource_group: &str,
) -> Result<(), ServiceResponse> {
    let exists =
        cosmos_collection_exists(account_name, database_name, collection_name, resource_group)?;

    if !exists {
        create_collection(account_name, database_name, collection_name, resource_group)?;
    }

    Ok(())
}

#[test]
pub fn send_text_message_test() {
    tokio::runtime::Runtime::new()
        .expect("Failed to create Tokio runtime")
        .block_on(init_env_logger(
            log::LevelFilter::Info,
            log::LevelFilter::Error,
        ));
    send_text_message(&SERVICE_CONFIG.test_phone_number, "this is a test")
        .expect("text message should be sent");
}

#[test]
pub fn send_email_test() {
    tokio::runtime::Runtime::new()
        .expect("Failed to create Tokio runtime")
        .block_on(init_env_logger(
            log::LevelFilter::Info,
            log::LevelFilter::Error,
        ));
    send_email(
        &SERVICE_CONFIG.test_email,
        &SERVICE_CONFIG.service_email,
        "this is a test",
        "test email",
    )
    .expect("text message should be sent");
}

pub fn cleanup_randomized_resource_groups() {
    let args = ["group", "list"];
    print_cmd(&args);

    let json_doc = exec_os(&args).expect("Failed to execute OS command");

    let parsed: serde_json::Value = serde_json::from_str(&json_doc).expect("Failed to parse JSON");

    // Assume the root of the parsed JSON is an array
    if let serde_json::Value::Array(groups) = parsed {
        for group in groups {
            if let Some(group_name) = group.get("name").and_then(|name| name.as_str()) {
                if group_name.contains("test-resource-group") {
                    delete_resource_group(group_name).expect("should be able to delete this group");
                }
            }
        }
    }
}

#[test]
pub fn azure_resources_integration_test() {
    let randomize_end_of_name = &format!("{}", rand::thread_rng().gen_range(100_000..=999_999));
    let resource_group = "test-resource-group-".to_owned() + randomize_end_of_name;
    let location = &SERVICE_CONFIG.azure_location;
    let kv_name = std::env::var("KEV_VAULT_NAME").expect("KEV_VAULT_NAME not found in environment");
    let cosmos_account_name = "test-cosmos-account-".to_owned() + randomize_end_of_name;
    let database_name = "test-cosmos-database-".to_owned() + randomize_end_of_name;
    let collection_name = "test-collection-".to_owned() + randomize_end_of_name;

    //
    //  run the async function synchronously
    tokio::runtime::Runtime::new()
        .expect("Failed to create Tokio runtime")
        .block_on(init_env_logger(
            log::LevelFilter::Info,
            log::LevelFilter::Error,
        ));

    // make sure the user is logged in

    verify_login_or_panic();

    // cleanup if any tests left something behind
    cleanup_randomized_resource_groups();

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
        &vec![Role::Admin],
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

#[test]
fn database_exits() {
    //
    //  run the async function synchronously
    tokio::runtime::Runtime::new()
        .expect("Failed to create Tokio runtime")
        .block_on(init_env_logger(
            log::LevelFilter::Info,
            log::LevelFilter::Error,
        ));

    let exists = cosmos_account_exists(
        &SERVICE_CONFIG.cosmos_account,
        &SERVICE_CONFIG.resource_group,
    )
    .expect("should work");

    assert!(exists);

    let exists = cosmos_account_exists(
        &SERVICE_CONFIG.cosmos_account,
        "TEST-RG-2193472304723089487",
    )
    .expect("should work - but not exist");

    assert!(!exists);

    let exists = cosmos_account_exists("bad account name", "TEST-RG-2193472304723089487")
        .expect("should work - but not exist");

    assert!(!exists);

    let exists = cosmos_database_exists(
        &SERVICE_CONFIG.cosmos_account,
        &SERVICE_CONFIG.cosmos_database_name,
        &SERVICE_CONFIG.resource_group,
    )
    .expect("should work");

    assert!(exists);

    let exists = cosmos_database_exists(
        &SERVICE_CONFIG.cosmos_account,
        "Very-Bad-Db-name",
        &SERVICE_CONFIG.resource_group,
    )
    .expect("should work - but no exist");

    assert!(!exists);
}
