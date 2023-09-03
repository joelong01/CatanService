#![allow(dead_code)]
use once_cell::sync::Lazy;
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::process::Command;
use std::str;
use std::sync::Mutex;

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

pub fn verify_login_or_panic() -> String {
    // Check if the user is already logged into Azure
    if let Some(subscription_id) = SUBSCRIPTION_ID.get() {
        return subscription_id.clone();
    }

    let mut cmd = Command::new("az");
    cmd.arg("account").arg("show");

    match exec_os(&mut cmd) {
        Ok(output) => {
            let response: Value = serde_json::from_str(&output)
                .unwrap_or_else(|_| panic!("Failed to parse JSON output from Azure CLI"));

            match response["id"].as_str() {
                Some(subscription_id) => {
                    println!("Already logged into Azure.");
                    subscription_id.to_string()
                }
                None => panic!("No subscription ID found in Azure CLI response."),
            }
        }
        Err(_) => {
            // If not logged in, prompt the user to log in
            println!("Not logged into Azure. Initiating login process...");
            let mut login_cmd = Command::new("az");
            login_cmd.arg("login");
            match exec_os(&mut login_cmd) {
                Ok(_) => {
                    println!("Login to Azure succeeded!");

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

static LOCATIONS_CACHE: Lazy<Mutex<Option<Vec<String>>>> = Lazy::new(|| Mutex::new(None));

pub fn is_location_valid(location: &str) -> Result<bool, String> {
    let mut cached_locations = LOCATIONS_CACHE.lock().unwrap();

    // Step 1: Check the cache for the location
    if let Some(ref locations) = *cached_locations {
        if locations.contains(&location.to_string()) {
            return Ok(true);
        }
    }

    // Step 2: If not in cache, fetch from Azure
    let mut command = Command::new("az");
    command.arg("account").arg("list-locations");
    match exec_os(&mut command) {
        Ok(output) => {
            let available_locations: Vec<Value> = serde_json::from_str(&output)
                .map_err(|e| format!("Error parsing locations: {}", e))?;

            // Step 3: Update the cache with fetched locations
            *cached_locations = Some(
                available_locations
                    .iter()
                    .filter_map(|loc| loc["name"].as_str())
                    .map(String::from)
                    .collect(),
            );

            if cached_locations
                .as_ref()
                .unwrap()
                .contains(&location.to_string())
            {
                return Ok(true);
            }
        }
        Err(e) => return Err(format!("Error executing Azure CLI: {}", e)),
    }

    Ok(false)
}

pub fn create_cosmosdb_account(
    resource_group_name: &str,
    db_name: &str,
    location: &str,
) -> Result<(), String> {
    if !is_location_valid(location)? {
        return Err(format!("Invalid location: {}", location));
    }

    if cosmosdb_account_exists(db_name, resource_group_name)? {
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
            println!("stdout: {}", output);
            Ok(())
        }
        Err(error) => Err(error),
    }
}
pub fn delete_cosmosdb_account(
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

fn cosmosdb_account_exists(
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

pub fn get_cosmosdb_secrets(
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
    if database_exists(account_name, database_name, resource_group)? {
        println!("Database {} already exists.", database_name);
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
            println!("Created database: {}", database_name);
            Ok(())
        }
        Err(error) => Err(error),
    }
}

fn database_exists(
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
    if collection_exists(account_name, database_name, collection_name, resource_group)? {
        println!("Collection {} already exists", collection_name);
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
            println!("Output: {}", output);
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

fn collection_exists(
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
//
//  create and delete keyvault isn't needed for the app -- key vault tends to be global to the company, not the app
//  and it's name has to be globally unique, so we instead use a keyvault name that is set in the environment. we
//  also assume that the app has been configured to access KeyVault.  When running in developer mode (outside azure)
//  the dev needs to be configured for access to KeyVault.
//
// pub fn create_keyvault(resource_group: &str, kv_name: &str, region: &str) -> Result<(), String> {
//     if keyvault_exists(kv_name)? {
//         return Ok(());
//     }

//     let mut command = Command::new("az");
//     command
//         .arg("keyvault")
//         .arg("create")
//         .arg("--name")
//         .arg(kv_name)
//         .arg("--resource-group")
//         .arg(resource_group)
//         .arg("--location") // The location parameter is required
//         .arg(region);

//     print_cmd(&command);

//     let status = command.status();

//     match status {
//         Ok(s) if s.success() => Ok(()),
//         Ok(_) => Err(format!(
//             "Failed to create Key Vault {} in resource group {}",
//             kv_name, resource_group
//         )),
//         Err(e) => Err(format!("Error executing Azure CLI: {}", e)),
//     }
// }

// pub fn delete_keyvault(kv_name: &str) -> Result<(), String> {
//     let mut command = Command::new("az");
//     command
//         .arg("keyvault")
//         .arg("delete")
//         .arg("--name")
//         .arg(kv_name);

//     print_cmd(&command);
//     let status = command.status();

//     match status {
//         Ok(s) if s.success() => Ok(()),
//         Ok(_) => Err(format!("Failed to delete Key Vault {}", kv_name)),
//         Err(e) => Err(format!("Error executing Azure CLI: {}", e)),
//     }
// }

pub fn print_cmd(command: &Command) {
    let program = command.get_program().to_string_lossy();
    let args = command.get_args();

    let cmd_str: Vec<String> = std::iter::once(program.into_owned())
        .chain(args.map(|arg| arg.to_string_lossy().into_owned()))
        .collect();

    println!("Executing: {}", cmd_str.join(" "));
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
        println!("KV {} already exists", kv_name);
        Ok(true)
    } else {
        println!("{} does not exist", kv_name);
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

// use crate::azure_setup::azure_wrapper::{
//     create_collection, create_cosmos_db_instance, create_keyvault, create_resource_group,
//     delete_resource_group, resource_group_exists, retrieve_cosmos_secrets,
//     store_cosmos_secrets_in_keyvault,
// };

#[test]
pub fn azure_resources_integration_test() {
    let three_letters = "abc";
    let resource_group = "test-resource-group-".to_owned() + three_letters;
    let location = "eastus"; // You can adjust this as needed
    let kv_name = std::env::var("KEV_VAULT_NAME").expect("KEV_VAULT_NAME not found in environment");
    let cosmos_account_name = "test-cosmos-account-".to_owned() + three_letters;
    let database_name = "test-cosmos-database-".to_owned() + three_letters;
    let collection_name = "test-collection-".to_owned() + three_letters;

    // make sure the user is logged in

    verify_login_or_panic();

    // verify KV exists
    keyvault_exists(&kv_name).expect(&format!("Failed to find Key Vault named {}.", kv_name));

    // Create a test resource group
    println!("creating resource group");
    create_resource_group(&resource_group, location).expect("Failed to create resource group.");

    println!("creating cosmosdb: {}", cosmos_account_name);
    //Add a Cosmos DB instance to it
    create_cosmosdb_account(&resource_group, &cosmos_account_name, location)
        .expect("Failed to create Cosmos DB instance.");

    println!("Creating database: {}", database_name);
    create_database(&cosmos_account_name, &database_name, &resource_group)
        .expect("creating a cosmos db should succeed");
    // Create a collection in the Cosmos DB instance
    println!("Creating collection: {}", collection_name);
    create_collection(
        &cosmos_account_name,
        &database_name,
        &collection_name,
        &resource_group,
    )
    .expect("Failed to create collection in Cosmos DB.");

    //Add the Cosmos DB secrets to Key Vault
    let secrets = get_cosmosdb_secrets(&cosmos_account_name, &resource_group)
        .expect("Failed to retrieve Cosmos DB secrets.");

    let secret = secrets
        .iter()
        .find(|s| s.key_kind == "Secondary")
        .expect("there should be a Secondary key in the cosmos secretes");

    //
    //  are they already there?
    let result = retrieve_cosmos_secrets_from_keyvault(&kv_name);

    match result {
        Ok(s) => assert_eq!(s, *secret),
        Err(_) => {}
    };

    store_cosmos_secrets_in_keyvault(&secret, &kv_name)
        .expect("Failed to store secrets in Key Vault.");

    //Validate that the resource group, Cosmos DB, and Key Vault exist
    assert!(
        resource_group_exists(&resource_group).expect("Failed to check resource group existence.")
    );
    assert!(
        cosmosdb_account_exists(&cosmos_account_name, &resource_group)
            .expect("Failed to check Cosmos DB existence.")
    );

    //  Get the secrets back out of Key Vault and validate they are correct
    let retrieved_secrets = retrieve_cosmos_secrets_from_keyvault(&kv_name)
        .expect("Failed to retrieve secrets from Key Vault.");
    assert_eq!(
        *secret, retrieved_secrets,
        "Stored and retrieved secrets do not match."
    );

    // Delete Cosmos DB
    delete_cosmosdb_account(&cosmos_account_name, &resource_group)
        .expect("Failed to delete Cosmos DB.");

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

    let token = create_jwt_token("test_id", "test@email.com", &current_login_key)
        .expect("create token should not fail");

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