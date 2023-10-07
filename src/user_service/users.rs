#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_imports)]

use actix_service::Service;
use actix_web::dev::Server;
use azure_core::request_options::User;
use bcrypt::{hash, verify};
use rand::Rng;
use url::form_urlencoded;

use crate::azure_setup::azure_wrapper::{
    cosmos_account_exists, cosmos_collection_exists, cosmos_database_exists, key_vault_get_secret,
    key_vault_save_secret, keyvault_exists, send_email, send_text_message, verify_login_or_panic,
};
use crate::cosmos_db::database_abstractions::DatabaseWrapper;
use crate::middleware::security_context::{KeyKind, SecurityContext};
use crate::middleware::service_config::SERVICE_CONFIG;
use crate::shared::service_models::{Claims, LoginHeaderData, PersistUser, Role};
/**
 * this module implements the WebApi to create the database/collection, list all the users, and to create/find/delete
 * a User document in CosmosDb
 */
use crate::trace_function;
use crate::user_service::user_handlers::find_user_by_id_handler;

use crate::games_service::long_poller::long_poller::LongPoller;

use crate::middleware::request_context_mw::RequestContext;
use crate::shared::shared_models::{
    GameError, ProfileStorage, ResponseType, ServiceError, UserProfile, UserType,
};

use reqwest::StatusCode;

/**
 * this sets up CosmosDb to make the sample run. the only prereq is the secrets set in
 * .devconainter/required-secrets.json, this API will call setupdb. this just calls the setupdb api and deals with errors
 *
 * you can't do the normal authn/authz here because the authn path requires the database to exist.  for this app,
 * the users database will be created out of band and this path is just for test users.
 */

pub async fn verify_cosmosdb(context: &RequestContext) -> Result<(), ServiceError> {
    trace_function!("verify_cosmosdb");
    let use_cosmos_db = match &context.claims {
        Some(claims) => {
            claims.profile_storage == ProfileStorage::CosmosDb
                || claims.profile_storage == ProfileStorage::CosmosDbTest
        }
        None => {
            return Err(ServiceError::new_unauthorized("Claims must be set"));
        }
    };

    if use_cosmos_db {
        if cosmos_account_exists(
            &context.config.cosmos_account,
            &context.config.resource_group,
        )
        .is_err()
        {
            return Err(ServiceError::new(
                &format!("account {} does not exist", context.config.cosmos_account),
                StatusCode::NOT_FOUND,
                ResponseType::NoData,
                GameError::HttpError,
            ));
        }

        if cosmos_database_exists(
            &context.config.cosmos_account,
            &context.config.cosmos_database_name,
            &context.config.resource_group,
        )
        .is_err()
        {
            return Err(ServiceError::new(
                &format!(
                    "account {} does exists, but database {} does not",
                    context.config.cosmos_account, context.config.cosmos_database_name
                ),
                StatusCode::NOT_FOUND,
                ResponseType::NoData,
                GameError::HttpError,
            ));
        }

        for collection in &context
            .database()?
            .as_user_db()
            .get_collection_names(context.is_test())
        {
            let collection_exists = cosmos_collection_exists(
                &context.config.cosmos_account,
                &context.config.cosmos_database_name,
                &collection,
                &context.config.resource_group,
            );

            if collection_exists.is_err() {
                let error_message = format!(
                    "account {} exists, database {} exists, but {} does not",
                    context.config.cosmos_account, context.config.cosmos_database_name, collection
                );

                return Err(ServiceError::new(
                    &error_message,
                    StatusCode::NOT_FOUND,
                    ResponseType::NoData,
                    GameError::HttpError,
                ));
            }
        }
    }
    Ok(())
}

async fn internal_register_user(
    password: &str,
    profile_in: &UserProfile,
    roles: &mut Vec<Role>,
    database: &DatabaseWrapper,
) -> Result<UserProfile, ServiceError> {
    let email = profile_in
        .pii
        .as_ref()
        .map(|pii| pii.email.clone())
        .ok_or_else(|| ServiceError::new_bad_request("no email specified"))?;

    if database
        .as_user_db()
        .find_user_by_email(&email)
        .await
        .is_ok()
    // you can't register twice!
    {
        return Err(ServiceError::new_conflict_error("User already exists"));
    }
    // this lets us bootstrap the system -- my assumption is that if you can set the environment variable, then you are
    // an admin.  You can have a test context so that you can create the admin in the mock database.
    if email == SERVICE_CONFIG.admin_email {
        roles.push(Role::Admin);
    }

    // Hash the password
    let password_hash = match hash(&password, bcrypt::DEFAULT_COST) {
        Ok(hp) => hp,
        Err(e) => {
            let err_message = format!("{:#?}", e);
            return Err(ServiceError::new(
                "Error Hashing Password",
                StatusCode::INTERNAL_SERVER_ERROR,
                ResponseType::ErrorInfo(err_message.to_owned()),
                GameError::HttpError,
            ));
        }
    };

    // Create the user record
    let mut persist_user = PersistUser::from_user_profile(&profile_in, password_hash.to_owned());
    // put the generated id into th user profile as well so that the client will see it
    persist_user.user_profile.user_id = Some(persist_user.id.clone());
    // ignore the game stats passed in by the client and create a new one
    persist_user.user_profile.games_played = Some(0);
    persist_user.user_profile.games_won = Some(0);
    persist_user.roles = roles.clone();
    database
        .as_user_db()
        .update_or_create_user(&persist_user)
        .await?;

    Ok(persist_user.user_profile)
}

///
/// this is the non-authenticated user.  Anybody can register, they just need to give us a password.  if they
/// are being registered it is *not* a test and goes to cosmosdb -- because if it is a test we user register_test_user
/// which must be authenticated and must be in the Admin role.
pub async fn register_user(
    password: &str,
    profile_in: &UserProfile,
    request_context: &RequestContext,
) -> Result<UserProfile, ServiceError> {
    //
    //  regular users are always in the "normal" cosmos collection
    let database = DatabaseWrapper::from_location(ProfileStorage::CosmosDb, &SERVICE_CONFIG);
    internal_register_user(password, profile_in, &mut vec![Role::User], &database).await
}

pub async fn update_profile(
    profile_in: &UserProfile,
    request_context: &RequestContext,
) -> Result<(), ServiceError> {
    let claims = request_context.claims.as_ref().unwrap();
    let database = request_context.database()?;
    let mut persist_user = database.as_user_db().find_user_by_id(&claims.id).await?;
    persist_user.update_profile(&profile_in);

    database
        .as_user_db()
        .update_or_create_user(&persist_user)
        .await?;
    Ok(())
}

///
/// the big difference between register_user and register_test_user is that the latter is authenticated and only
/// somebody in the admin group can add a test user.
pub async fn register_test_user(
    password: &str,
    profile_in: &UserProfile,
    request_context: &RequestContext,
) -> Result<UserProfile, ServiceError> {
    if !request_context.is_caller_in_role(Role::Admin) {
        return Err(ServiceError::new_unauthorized(""));
    }

    //
    //  always put test users in the test db
    let database = DatabaseWrapper::from_location(ProfileStorage::CosmosDbTest, &SERVICE_CONFIG);

    let mut profile = profile_in.clone();
    profile.display_name = format!("{}: [Test]", profile.display_name);

    internal_register_user(
        password,
        &profile,
        &mut vec![Role::User, Role::TestUser],
        &database,
    )
    .await
}

/**
 * login to the system.
 * a cleartext password is passed in (depending on HTTPS to stop MitM attacks and encrypt payload)
 * find the user in the database
 * hash the password and make sure it matches the hash in the db
 * if it does, return a signed JWT token
 * add the user to the ALL_USERS_MAP
 */
pub async fn login(
    login_data: &LoginHeaderData,
    request_context: &RequestContext,
) -> Result<String, ServiceError> {
    //
    //  because this call is non-authenticated, the database will always be cosmos db because the db choice is set
    // in the claims, which are created in this function.
    let database =
        DatabaseWrapper::from_location(login_data.profile_location.clone(), &SERVICE_CONFIG);

    let user = database
        .as_user_db()
        .find_user_by_email(&login_data.user_name)
        .await?;

    let password_hash: String = match user.password_hash {
        Some(p) => p,
        None => {
            return Err(ServiceError::new(
                "user document does not contain a password hash",
                StatusCode::INTERNAL_SERVER_ERROR,
                ResponseType::NoData,
                GameError::HttpError,
            ));
        }
    };
    let result = verify(&login_data.password, &password_hash);
    let is_password_match = match result {
        Ok(m) => m,
        Err(e) => {
            return Err(ServiceError::new(
                &format!("Error from bcrypt library: {:#?}", e),
                StatusCode::INTERNAL_SERVER_ERROR,
                ResponseType::NoData,
                GameError::HttpError,
            ));
        }
    };

    if is_password_match {
        let claims = Claims::new(
            &user.id,
            &login_data.user_name,
            24 * 60 * 60,
            &user.roles,
            login_data.profile_location.clone(),
        );
        let token_result = request_context
            .security_context
            .login_keys
            .sign_claims(&claims);
        match token_result {
            Ok(token) => Ok(token),
            Err(e) => {
                return Err(ServiceError::new(
                    "Error Hashing token",
                    StatusCode::INTERNAL_SERVER_ERROR,
                    ResponseType::ErrorInfo(format!("{:#?}", e)),
                    GameError::HttpError,
                ));
            }
        }
    } else {
        return Err(ServiceError::new_unauthorized(""));
    }
}

/**
 *  this will get a list of all documents.  Note this does *not* do pagination. This would be a reasonable next step to
 *  show in the sample
 */
pub async fn list_users(
    request_context: &RequestContext,
) -> Result<Vec<UserProfile>, ServiceError> {
    // Get list of users
    match request_context.database()?.as_user_db().list().await {
        Ok(users) => {
            let user_profiles: Vec<UserProfile> = users
                .iter()
                .map(|user| UserProfile::from_persist_user(&user))
                .collect();

            Ok(user_profiles)
        }
        Err(err) => {
            return Err(ServiceError::new(
                "",
                StatusCode::NOT_FOUND,
                ResponseType::ErrorInfo(format!("Failed to retrieve user list: {}", err)),
                GameError::HttpError,
            ));
        }
    }
}
///
///     1. email should be id or email
///         - figure out which one it is by context
///     2. check the claims -- you can always look up your own profile
///     3. an admin can look up anybody's profile
pub async fn get_profile(
    id_or_email: &str,
    request_context: &RequestContext,
) -> Result<UserProfile, ServiceError> {
    let lookup_value: String;

    let user_id = request_context
        .claims
        .as_ref()
        .expect("auth_mw should have added this or rejected the call")
        .id
        .clone();

    let user_email = request_context
        .claims
        .as_ref()
        .expect("auth_mw should have added this or rejected the call")
        .sub
        .clone();

    //
    //  so there can be 3 things to lokup by.  if Self, look in the context
    //  and lookup by that id.  if it has an @ in it, look up by email.
    //  only an admin can look up somebody else's profile.

    if id_or_email.to_ascii_lowercase() == "self" {
        lookup_value = user_id.clone().clone();
    } else {
        lookup_value = id_or_email.to_string();
    }

    // lookup value is either id or email...is it different than the one in the context?

    if lookup_value != user_id
        && lookup_value != user_email
        && !request_context.is_caller_in_role(Role::Admin)
    {
        // if you aren't the admin, you can only look up your own profile
        return Err(ServiceError::new_unauthorized(""));
    }

    let user = match lookup_value.contains("@") {
        true => {
            request_context
                .database()?
                .as_user_db()
                .find_user_by_email(&lookup_value)
                .await?
        }
        false => {
            request_context
                .database()?
                .as_user_db()
                .find_user_by_id(&lookup_value)
                .await?
        }
    };

    Ok(UserProfile::from_persist_user(&user))
}

pub async fn delete(id: &str, request_context: &RequestContext) -> Result<(), ServiceError> {
    let user_id = request_context
        .claims
        .as_ref()
        .expect("auth_mw should have added this or rejected the call")
        .id
        .clone();

    if user_id != id && !request_context.is_caller_in_role(Role::Admin) {
        return Err(ServiceError::new_unauthorized(
            "only an admin can delete another user",
        ));
    }

    let result = request_context
        .database()?
        .as_user_db()
        .delete_user(id)
        .await;

    match result {
        Ok(..) => Ok(()),
        Err(err) => {
            return Err(ServiceError::new(
                "failed to delete user",
                StatusCode::BAD_REQUEST,
                ResponseType::NoData,
                GameError::HttpError,
            ))
        }
    }
}
///
/// the token is a signed claim with an email
/// validate it
/// get the profile
/// mark the email as validated
/// save the profile
pub async fn validate_email(token: &str) -> Result<(), ServiceError> {
    trace_function!("validate_email");
    let decoded_token = form_urlencoded::parse(token.as_bytes())
        .map(|(key, _)| key)
        .collect::<Vec<_>>()
        .join("");

    let security_context = SecurityContext::cached_secrets();
    let claims = match security_context
        .validation_keys
        .validate_token(&decoded_token)
    {
        Some(c) => c,
        None => return Err(ServiceError::new_unauthorized("")),
    };

    let request_context =
        RequestContext::new(Some(&claims), &None, &SERVICE_CONFIG, &security_context);

    let id = claims.id.clone(); // Borrowing here.
    let database = request_context.database()?; //rust requires us to declare this with a let
    let user_db = database.as_user_db(); // so that database defines the lifetime of user_db
    let mut user = user_db.find_user_by_id(&id).await?;
    user.user_profile.validated_email = true;
    user_db.update_or_create_user(&user).await?;
    Ok(())
}

//
//  url is in the form of host://api/v1/users/validate-email/<token>
pub fn get_validation_url(host: &str, claims: &Claims, request_context: &RequestContext) -> String {
    let validation_claims = claims.into_validation_claims();
    let token = request_context
        .security_context
        .validation_keys
        .sign_claims(&validation_claims)
        .expect("Token creation should not fail");

    let encoded_token = form_urlencoded::byte_serialize(token.as_bytes()).collect::<String>();

    format!(
        "https://{}/api/v1/users/validate-email/{}",
        host, encoded_token
    )
}
///
/// Send a validation email
/// returns an error or a ServiceResponse that has the validation URL embedded in it.  RegistgerUser should call
/// this and drop the Ok() response. the Ok() response is useful for the test cases.
pub fn send_validation_email(request_context: &RequestContext) -> Result<(), ServiceError> {
    trace_function!("send_validation_email");
    let host_name = std::env::var("HOST_NAME").expect("HOST_NAME must be set");
    let claims = request_context
        .claims
        .clone()
        .expect("claims are set by auth middleware, or the call is rejected");
    let url = get_validation_url(&host_name, &claims, &request_context);
    let msg = format!(
        "Thank you for registering with our Service.\n\n\
         Click on this link to validate your email: {}\n\n\
         If you did not register with the service, something has gone terribly wrong.",
        url
    );
    send_email(
        &claims.sub,
        &SERVICE_CONFIG.service_email,
        "Please validate your email",
        &msg,
    )
}
///
/// 1. get the user profile
/// 2. generate a random 6 digit number
/// 3. store the number in the profile
/// 4. update the profile
/// 5. send the text message to the phone
pub async fn send_phone_code(request_context: &RequestContext) -> Result<(), ServiceError> {
    let user_id = request_context
        .claims
        .as_ref()
        .expect("auth_mw should have added this or rejected the call")
        .id
        .clone();

    let random_code: i32 = rand::thread_rng().gen_range(100_000..=999_999);

    let code: i32 = request_context
        .test_context
        .as_ref()
        .and_then(|test_ctx| test_ctx.phone_code.as_ref())
        .copied()
        .unwrap_or(random_code);

    let sr = internal_send_phone_code(&user_id, code, request_context).await;

    sr
}
///
/// I have the send_phone_code() and the internal_send_phone_code() so that i can test internal_send_phone_code() in an
/// automated way (by having the test create the code and pass it in and then pass it to validate_phone())
async fn internal_send_phone_code(
    user_id: &str,
    code: i32,
    request_context: &RequestContext,
) -> Result<(), ServiceError> {
    let database = request_context.database()?; //rust requires us to declare this with a let
    let user_db = database.as_user_db(); // so that database defines the lifetime of user_db
    let mut persist_user = user_db.find_user_by_id(user_id).await?;

    let phone_number = match &persist_user.user_profile.pii {
        Some(pii) => pii.phone_number.clone(),
        None => return Err(ServiceError::new_bad_request("no phone number in profile")),
    };

    persist_user.phone_code = Some(code.to_string());
    user_db.update_or_create_user(&persist_user).await?;
    let msg = format!(
        "This is your 6 digit code for the Catan Service. \
                   If You did not request this code, ignore this message. \
                   code: {}",
        code
    );
    send_text_message(&phone_number, &msg)
}

/// Validates a phone code for a given user.
///
/// This function checks if the provided phone code matches the stored code for the user.
/// If the code matches, it updates the user's information to indicate a validated phone.
///
/// # Arguments
///

/// * `code` - The phone code to validate.
/// * `request_context` - Context related to the current request.
///
/// # Returns
///
/// * `Ok(ServiceResponse)` if the phone code is validated successfully.
/// * `Err(ServiceResponse)` if the code does not match or is missing.
pub async fn validate_phone(
    code: &str,
    request_context: &RequestContext,
) -> Result<(), ServiceError> {
    let database = request_context.database()?; //rust requires us to declare this with a let
    let user_db = database.as_user_db(); // so that database defines the lifetime of user_db
    let user_id = request_context
        .claims
        .as_ref()
        .expect("auth_mw should have added this or rejected the call")
        .id
        .clone();

    let mut persist_user = user_db.find_user_by_id(&user_id).await?;

    match &persist_user.phone_code {
        // If the stored code matches the provided code, validate the phone.
        Some(c) if c.to_string() == code => {
            persist_user.user_profile.validated_phone = true;
            persist_user.phone_code = None;
            user_db.update_or_create_user(&persist_user).await?;
            Ok(())
        }
        // Handle all other cases (mismatch or missing code) as errors.
        _ => Err(ServiceError::new(
            "incorrect code.  request a new one",
            reqwest::StatusCode::BAD_REQUEST,
            ResponseType::NoData,
            GameError::HttpError,
        )),
    }
}

///
/// rotates the login keys -- admin only
///

pub async fn rotate_login_keys(request_context: &RequestContext) -> Result<(), ServiceError> {
    panic!("update this to match the new SecurityContext naming convention");
    // if !request_context.is_caller_in_role(Role::Admin) {
    //     return Err(ServiceError::new_unauthorized_error(""));
    // }

    // let kv_name = request_context.config.kv_name.to_owned();
    // let old_name = "oldLoginSecret-test";
    // let new_name = "currentLoginSecret-test";

    // // make sure the user/service is logged in
    // verify_login_or_panic();

    // // verify KV exists
    // keyvault_exists(&kv_name).expect(&format!("Failed to find Key Vault named {}.", kv_name));

    // let current_primary_login_key = key_vault_get_secret(&kv_name, KeyKind::PRIMARY_KEY)?;

    // let new_key = SecurityContext::generate_jwt_key();

    // key_vault_save_secret(&kv_name, KeyKind::SECONDARY_KEY, &current_primary_login_key)?;
    // key_vault_save_secret(&kv_name, KeyKind::PRIMARY_KEY, &new_key)?;

    // return new_ok_response!("");
}

pub async fn find_user_by_id(
    id: &str,
    request_context: &RequestContext,
) -> Result<UserProfile, ServiceError> {
    let claims_id = request_context
        .claims
        .as_ref()
        .expect("auth_mw should have added this or rejected the call")
        .id
        .clone();

    if claims_id != *id && !request_context.is_caller_in_role(Role::Admin) {
        return Err(ServiceError::new(
            "you can't peak at somebody else's profile!",
            StatusCode::UNAUTHORIZED,
            ResponseType::NoData,
            GameError::HttpError,
        ));
    }

    let persist_user = request_context
        .database()?
        .as_user_db()
        .find_user_by_id(&id)
        .await?;

    Ok(UserProfile::from_persist_user(&persist_user))
}

/// a "local user" is a user that can play in the game, but does not participate in the long polling. instead messages
/// for a local user are sent to the creator's long poller.  Having local users means that you can have a full game
/// played from only one computer - i do this with my friends where we project the game onto a give 4k tv and use the
/// physical games assets (cards, dice, etc.) to play the game.  see http://github.com/joelong01/Catan or
/// http://github.com/joelong01/CatanTs
///
/// local users are linked to ConnectedUsers via the PersisProfile via local_user_owner_id
/// local users have no PII
///
pub async fn create_local_user(
    profile_in: &UserProfile,
    request_context: &RequestContext,
) -> Result<(), ServiceError> {
    let user_id = request_context
        .claims
        .as_ref()
        .expect("auth_mw should have added this or rejected the call")
        .id
        .clone();
    let mut profile = profile_in.clone();
    profile.user_id = Some(PersistUser::new_id());
    profile.pii = None;
    profile.user_type = UserType::Local;
    profile.games_played = Some(0);
    profile.games_won = Some(0);
    profile.display_name = format!("{} [Local]", profile.display_name);
    //
    //  create a hash that is extremely unlikely to be guessed so that the user can't login

    let persist_user = PersistUser::from_local_user(&user_id, &profile);

    request_context
        .database()?
        .as_user_db()
        .update_or_create_user(&persist_user)
        .await?;
    Ok(())
}

///
/// only an admin or the ConnectedUser can update the LocalUser
pub async fn update_local_user(
    new_profile: &UserProfile,
    request_context: &RequestContext,
) -> Result<(), ServiceError> {
    // get the user_db connection
    let database = request_context.database()?; //rust requires us to declare this with a let
    let user_db = database.as_user_db(); // so that database defines the lifetime of user_db
                                         // Check that PII is not filled in for local users
    if new_profile.pii.is_some() {
        return Err(ServiceError::new_bad_request("Local Users have no PII!"));
    }

    // Check that UserType is 'Local'
    if new_profile.user_type != UserType::Local {
        return Err(ServiceError::new_bad_request("UserType must be 'Local'"));
    }

    // Ensure user_id is set in the profile
    let local_user_id = new_profile
        .user_id
        .as_ref()
        .ok_or_else(|| ServiceError::new_bad_request("user_id must be set in profile"))?;

    // Get the authenticated user's ID from claims
    let id_in_claims = request_context
        .claims
        .as_ref()
        .expect("auth_mw should have added this or rejected the call")
        .id
        .clone();

    // Find the local user by ID
    let mut local_user = user_db.find_user_by_id(local_user_id).await?;

    // Ensure the local user is connected to a connected user
    let connection_id = local_user
        .connected_user_id
        .as_ref()
        .ok_or_else(|| ServiceError::new_bad_request("id does not correspond to a local user"))?
        .clone();

    // Check if the local user isn't "connected" to the connected caller and the caller isn't an admin
    if connection_id != id_in_claims && !request_context.is_caller_in_role(Role::Admin) {
        return Err(ServiceError::new_unauthorized(
            "only an admin can delete another user",
        ));
    }

    // Update the profile
    local_user.update_profile(new_profile);

    // Save the updated user
    user_db.update_or_create_user(&local_user).await?;
    Ok(())
}

///
/// only an admin or the ConnectedUser can delete the LocalUser
/// we use the passed in PK of the local user to look up its profile. then we look at the connected_user value and
/// make sure it is the PK of the signed in user.  if so, it can be deleted.
///
pub async fn delete_local_user(
    local_user_primary_key: &str,
    request_context: &RequestContext,
) -> Result<(), ServiceError> {
    let database = request_context.database()?; //rust requires us to declare this with a let
    let user_db = database.as_user_db(); // so that database defines the lifetime of user_db

    let local_user_profile = user_db.find_user_by_id(&local_user_primary_key).await?;

    if local_user_profile.connected_user_id.is_none() {
        return Err(ServiceError::new_not_found(
            "invalid local user id",
            local_user_primary_key,
        ));
    }

    let id_in_claims = request_context
        .claims
        .as_ref()
        .expect("auth_mw should have added this or rejected the call")
        .id
        .clone();

    let connected_id = match local_user_profile.connected_user_id {
        Some(id) => id,
        None => {
            return Err(ServiceError::new_not_found(
                "no connected id for input",
                local_user_primary_key,
            ));
        }
    };

    if id_in_claims == connected_id || request_context.is_caller_in_role(Role::Admin) {
        user_db.delete_user(&local_user_primary_key).await?;
        return Ok(());
    }

    return Err(ServiceError::new_unauthorized(""));
}

pub async fn get_local_users(
    connected_user_id: &str,
    request_context: &RequestContext,
) -> Result<Vec<UserProfile>, ServiceError> {
    let id_in_claims = request_context
        .claims
        .as_ref()
        .expect("auth_mw should have added this or rejected the call")
        .id
        .clone();

    let connected_id = if connected_user_id == "Self" {
        id_in_claims.clone()
    } else {
        connected_user_id.to_string()
    };
    if id_in_claims == connected_id || request_context.is_caller_in_role(Role::Admin) {
        let users = request_context
            .database()?
            .as_user_db()
            .get_connected_users(&connected_id)
            .await?;
        let user_profiles: Vec<UserProfile> = users
            .iter()
            .map(|user| UserProfile::from_persist_user(&user))
            .collect();
        return Ok(user_profiles);
    };

    return Err(ServiceError::new_unauthorized(""));
}
#[cfg(test)]
mod tests {

    use tracing::info;

    use crate::{
        create_test_service, full_info, init_env_logger,
        middleware::request_context_mw::TestCallContext,
        test::{test_helpers::test::*, test_proxy::TestProxy},
    };

    use super::*;

    // Test the login function
    #[tokio::test]
    async fn test_login_mocked() {
        init_env_logger(log::LevelFilter::Info, log::LevelFilter::Error).await;
        test_login(false).await;
    }

    #[tokio::test]
    async fn test_local_users() {
        init_env_logger(log::LevelFilter::Info, log::LevelFilter::Error).await;
        let app = create_test_service!();
        let code = 569342;
        let mut proxy = TestProxy::new(&app);

        let auth_token = TestHelpers::delete_all_test_users(&mut proxy).await;
        let users = TestHelpers::register_test_users(&mut proxy, Some(auth_token)).await;
        //
        // i'm logged in as the admin
        let last_id = users
            .last()
            .and_then(|user| user.user_id.as_ref())
            .map(|id| id.clone())
            .unwrap_or_else(|| panic!("UserProfile should have an id!"));

        full_info!("deleting user: {}", &last_id);
        proxy.delete_local_user(&last_id).await.expect("success");

        full_info!("logging in as test user");
        // login as the test user that is going to create the local user and set the auth token in the proxy
        let first_user = users.first().unwrap().clone();
        let login_data =
            LoginHeaderData::test_default(&first_user.get_email_or_panic(), "password");
        let test_token = proxy.login(&login_data).await.expect("success");

        proxy.set_auth_token(Some(test_token));

        full_info!("creating local users");
        // now we are logged in as the first test user...create a test local user connected to the first user
        proxy
            .create_local_user(&users.last().unwrap().clone())
            .await
            .expect("success");

        for i in 0..3 {
            let service_error = proxy
                .create_local_user(&UserProfile::new_test_user(None))
                .await
                .expect("success");
        }

        full_info!("getting local users");
        let local_user_profiles = proxy.get_local_users("Self").await.expect("success");
        assert_eq!(local_user_profiles.len(), 4);

        let first_id = users
            .first()
            .and_then(|user| user.user_id.as_ref())
            .map(|id| id.clone())
            .unwrap_or_else(|| panic!("UserProfile should have an id!"));

        full_info!("deleting local users");
        proxy.delete_local_user(&first_id).await.expect("success");

        let first_id = local_user_profiles
            .first()
            .and_then(|user| user.user_id.as_ref())
            .map(|id| id.clone())
            .unwrap_or_else(|| panic!("UserProfile should have an id!"));

        proxy.delete_local_user(&first_id).await.expect("success");

        let local_user_profiles = proxy.get_local_users("Self").await.expect("success");
        assert_eq!(local_user_profiles.len(), 3);
        // testing update_local_user
        full_info!("updating local users");
        let mut first_profile = local_user_profiles
            .first()
            .expect("we know this has 3 elemements in it")
            .clone();

        first_profile.games_won = Some(1);
        proxy
            .update_local_user(&first_profile)
            .await
            .expect("success");
        //
        //   get the profiles
        let local_user_profiles = proxy.get_local_users("Self").await.expect("success");
        assert_eq!(local_user_profiles.len(), 3);
    }

    #[tokio::test]
    async fn test_login_cosmos() {
        init_env_logger(log::LevelFilter::Info, log::LevelFilter::Error).await;
        test_login(true).await;
    }

    async fn test_login(use_cosmos: bool) {
        let profile_location = if use_cosmos {
            ProfileStorage::CosmosDbTest
        } else {
            ProfileStorage::MockDb
        };
        let token = TestHelpers::admin_login().await;
        let profile = UserProfile::new_test_user(None);
        let request_context = RequestContext::admin_default(&profile);
        let user_profile = register_test_user("password", &profile, &request_context)
            .await
            .expect("success");

        let database = DatabaseWrapper::from_location(profile_location.clone(), &SERVICE_CONFIG);
        database
            .as_user_db()
            .find_user_by_email(&user_profile.pii.unwrap().email)
            .await
            .expect("we just put this here!");

        let login_header = LoginHeaderData::new(
            &profile.get_email_or_panic(),
            "password",
            profile_location.clone(),
        );

        // Test login with correct credentials
        let response = login(&login_header, &request_context)
            .await
            .expect("login should succeed");

        // Test login with incorrect credentials
        let login_header = LoginHeaderData::new(
            &profile.get_email_or_panic(),
            "bad_password",
            profile_location.clone(),
        );
        let response = login(&login_header, &request_context).await;
        match response {
            Ok(_) => panic!("this should be an error!"),
            Err(e) => {
                assert_eq!(e.status, StatusCode::UNAUTHORIZED);
            }
        }

        // find user

        //  let user = find_user_by_id(id, is_test, &data)
    }

    // Similar tests for other functions: get_profile, find_user_by_id

    // Test JWT token creation and validation
    #[tokio::test]
    async fn test_jwt_token_creation_and_validation() {
        let claims = Claims::new(
            "user_id",
            "user_email@email.com",
            10 * 60,
            &vec![Role::Validation],
            ProfileStorage::CosmosDb,
        );
        let request_context = RequestContext::test_default(false);
        let token = request_context
            .security_context
            .login_keys
            .sign_claims(&claims)
            .unwrap();
        let token_claims = request_context
            .security_context
            .login_keys
            .validate_token(&token)
            .unwrap();
        assert_eq!(token_claims, claims);
    }
}
