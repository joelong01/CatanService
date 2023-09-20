#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_imports)]

use bcrypt::{hash, verify};
use rand::Rng;
use url::form_urlencoded;

use crate::azure_setup::azure_wrapper::{
    cosmos_account_exists, cosmos_collection_exists, cosmos_database_exists, key_vault_get_secret,
    key_vault_save_secret, keyvault_exists, send_email, send_text_message, verify_login_or_panic,
};
use crate::middleware::security_context::{KeyKind, SecurityContext};
use crate::middleware::service_config::SERVICE_CONFIG;
use crate::shared::service_models::{Claims, PersistUser, Role};
use crate::user_service::user_handlers::find_user_by_id_handler;
/**
 * this module implements the WebApi to create the database/collection, list all the users, and to create/find/delete
 * a User document in CosmosDb
 */
use crate::{bad_request_from_string, new_ok_response, new_unauthorized_response, trace_function};

use crate::games_service::long_poller::long_poller::LongPoller;

use crate::middleware::request_context_mw::RequestContext;
use crate::shared::shared_models::{
    GameError, ResponseType, ServiceResponse, UserProfile, UserType,
};

use reqwest::StatusCode;

/**
 * this sets up CosmosDb to make the sample run. the only prereq is the secrets set in
 * .devconainter/required-secrets.json, this API will call setupdb. this just calls the setupdb api and deals with errors
 *
 * you can't do the normal authn/authz here because the authn path requires the database to exist.  for this app,
 * the users database will be created out of band and this path is just for test users.
 */

pub async fn verify_cosmosdb(context: &RequestContext) -> Result<ServiceResponse, ServiceResponse> {
    trace_function!("setup");
    let use_cosmos_db = match &context.test_context {
        Some(tc) => tc.use_cosmos_db,
        None => {
            return new_unauthorized_response!("Test Header must be set");
        }
    };

    if use_cosmos_db {
        if cosmos_account_exists(
            &context.config.cosmos_account,
            &context.config.resource_group,
        )
        .is_err()
        {
            return Err(ServiceResponse::new(
                &format!("account {} does not exist", context.config.cosmos_account),
                StatusCode::NOT_FOUND,
                ResponseType::NoData,
                GameError::HttpError(StatusCode::NOT_FOUND),
            ));
        }

        if cosmos_database_exists(
            &context.config.cosmos_account,
            &context.config.cosmos_database_name,
            &context.config.resource_group,
        )
        .is_err()
        {
            return Err(ServiceResponse::new(
                &format!(
                    "account {} does exists, but database {} does not",
                    context.config.cosmos_account, context.config.cosmos_database_name
                ),
                StatusCode::NOT_FOUND,
                ResponseType::NoData,
                GameError::HttpError(StatusCode::NOT_FOUND),
            ));
        }

        for collection in &context.database.get_collection_names(context.is_test()) {
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

                return Err(ServiceResponse::new(
                    &error_message,
                    StatusCode::NOT_FOUND,
                    ResponseType::NoData,
                    GameError::HttpError(StatusCode::NOT_FOUND),
                ));
            }
        }
    }
    Ok(ServiceResponse::new(
        "already exists",
        StatusCode::ACCEPTED,
        ResponseType::NoData,
        GameError::NoError(String::default()),
    ))
}

async fn internal_register_user(
    password: &str,
    profile_in: &UserProfile,
    roles: &mut Vec<Role>,
    request_context: &RequestContext,
) -> Result<ServiceResponse, ServiceResponse> {
    let email = match &profile_in.pii {
        Some(pii) => pii.email.clone(),
        None => return Err(bad_request_from_string!("no email specified")),
    };

    if request_context
        .database
        .find_user_by_email(&email)
        .await
        .is_ok()
    // you can't register twice!
    {
        return Err(ServiceResponse::new(
            "User already exists",
            StatusCode::CONFLICT,
            ResponseType::NoData,
            GameError::HttpError(StatusCode::CONFLICT),
        ));
    }
    // this lets us bootstrap the system -- my assumption is that if you can set the environment variable, then you are
    // an admin.  You can have a test context so that you can create the admin in the mock database.
    if email == request_context.config.admin_email {
        roles.push(Role::Admin);
    }

    // Hash the password
    let password_hash = match hash(&password, bcrypt::DEFAULT_COST) {
        Ok(hp) => hp,
        Err(e) => {
            let err_message = format!("{:#?}", e);
            return Err(ServiceResponse::new(
                "Error Hashing Password",
                StatusCode::INTERNAL_SERVER_ERROR,
                ResponseType::ErrorInfo(err_message.to_owned()),
                GameError::HttpError(StatusCode::INTERNAL_SERVER_ERROR),
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
    request_context
        .database
        .update_or_create_user(&persist_user)
        .await
}

/// Registers a new user by hashing the provided password and creating a `PersistUser` record in the database.
///
/// # Arguments
///
/// * `profile_in` - UserProfile object
/// * `data` - `ServiceEnvironmentContext` data.
/// * `is_test` - test header set?
/// & `pwd_header_val` - the Option<> for the HTTP header containing the passwrod
///
/// # Returns
/// Body contains a ClientUser (an id + profile)
/// Returns an ServiceResponse indicating the success or failure of the registration process.
pub async fn register(
    password: &str,
    profile_in: &UserProfile,
    request_context: &RequestContext,
) -> Result<ServiceResponse, ServiceResponse> {
    if request_context.is_test() {
        return new_unauthorized_response!(
            "can't create a test user through this api.  use register-test-user"
        );
    }
    internal_register_user(password, profile_in, &mut vec![Role::User], request_context).await
}

pub async fn update_profile(
    profile_in: &UserProfile,
    request_context: &RequestContext,
) -> Result<ServiceResponse, ServiceResponse> {
    let claims = request_context.claims.as_ref().unwrap();

    let mut persist_user = request_context.database.find_user_by_id(&claims.id).await?;
    persist_user.update_profile(&profile_in);

    request_context
        .database
        .update_or_create_user(&persist_user)
        .await
}

///
/// the big difference between register_user and register_test_user is that the latter is authenticated and only
/// somebody in the admin group can add a test user.
pub async fn register_test_user(
    password: &str,
    profile_in: &UserProfile,
    request_context: &RequestContext,
) -> Result<ServiceResponse, ServiceResponse> {
    if !request_context.is_caller_in_role(Role::Admin) {
        return new_unauthorized_response!("");
    }
    internal_register_user(
        password,
        profile_in,
        &mut vec![Role::User, Role::TestUser],
        request_context,
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
    username: &str,
    password: &str,
    request_context: &RequestContext,
) -> Result<ServiceResponse, ServiceResponse> {
    let user = request_context
        .database
        .find_user_by_email(username)
        .await?;

    let password_hash: String = match user.password_hash {
        Some(p) => p,
        None => {
            return Err(ServiceResponse::new(
                "user document does not contain a password hash",
                StatusCode::INTERNAL_SERVER_ERROR,
                ResponseType::NoData,
                GameError::HttpError(StatusCode::INTERNAL_SERVER_ERROR),
            ));
        }
    };
    let result = verify(password, &password_hash);
    let is_password_match = match result {
        Ok(m) => m,
        Err(e) => {
            return Err(ServiceResponse::new(
                &format!("Error from bcrypt library: {:#?}", e),
                StatusCode::INTERNAL_SERVER_ERROR,
                ResponseType::NoData,
                GameError::HttpError(StatusCode::INTERNAL_SERVER_ERROR),
            ));
        }
    };

    if is_password_match {
        let claims = Claims::new(
            &user.id,
            username,
            24 * 60 * 60,
            &user.roles,
            &request_context.test_context,
        );
        let token_result = request_context
            .security_context
            .login_keys
            .sign_claims(&claims);
        match token_result {
            Ok(token) => {
                let _ = LongPoller::add_user(&user.id, &user.user_profile).await;
                Ok(ServiceResponse::new(
                    "",
                    StatusCode::OK,
                    ResponseType::Token(token),
                    GameError::NoError("ok".to_owned()),
                ))
            }
            Err(e) => {
                return Err(ServiceResponse::new(
                    "Error Hashing token",
                    StatusCode::INTERNAL_SERVER_ERROR,
                    ResponseType::ErrorInfo(format!("{:#?}", e)),
                    GameError::HttpError(StatusCode::INTERNAL_SERVER_ERROR),
                ));
            }
        }
    } else {
        return new_unauthorized_response!("");
    }
}

/**
 *  this will get a list of all documents.  Note this does *not* do pagination. This would be a reasonable next step to
 *  show in the sample
 */
pub async fn list_users(
    request_context: &RequestContext,
) -> Result<ServiceResponse, ServiceResponse> {
    // Get list of users
    match request_context.database.list().await {
        Ok(users) => {
            let client_users: Vec<UserProfile> = users
                .iter()
                .map(|user| UserProfile::from_persist_user(&user))
                .collect();

            Ok(ServiceResponse::new(
                "",
                StatusCode::OK,
                ResponseType::Profiles(client_users),
                GameError::NoError(String::default()),
            ))
        }
        Err(err) => {
            return Err(ServiceResponse::new(
                "",
                StatusCode::NOT_FOUND,
                ResponseType::ErrorInfo(format!("Failed to retrieve user list: {}", err)),
                GameError::HttpError(StatusCode::NOT_FOUND),
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
) -> Result<ServiceResponse, ServiceResponse> {
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
        return new_unauthorized_response!("");
    }

    let user_email = request_context
        .claims
        .as_ref()
        .expect("auth_mw should have added this or rejected the call")
        .sub
        .clone();

    let user = match lookup_value.contains("@") {
        true => {
            request_context
                .database
                .find_user_by_email(&lookup_value)
                .await?
        }
        false => {
            request_context
                .database
                .find_user_by_id(&lookup_value)
                .await?
        }
    };

    Ok(ServiceResponse::new(
        "",
        StatusCode::OK,
        ResponseType::Profile(UserProfile::from_persist_user(&user)),
        GameError::NoError(String::default()),
    ))
}

pub async fn delete(
    id: &str,
    request_context: &RequestContext,
) -> Result<ServiceResponse, ServiceResponse> {
    let user_id = request_context
        .claims
        .as_ref()
        .expect("auth_mw should have added this or rejected the call")
        .id
        .clone();

    if user_id != id && !request_context.is_caller_in_role(Role::Admin) {
        return new_unauthorized_response!("only an admin can delete another user");
    }

    let result = request_context.database.delete_user(id).await;

    match result {
        Ok(..) => Ok(ServiceResponse::new(
            &format!("deleted user with id: {}", user_id),
            StatusCode::OK,
            ResponseType::NoData,
            GameError::NoError(String::default()),
        )),
        Err(err) => {
            return Err(ServiceResponse::new(
                "failed to delete user",
                StatusCode::BAD_REQUEST,
                ResponseType::NoData,
                GameError::HttpError(StatusCode::BAD_REQUEST),
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
pub async fn validate_email(token: &str) -> Result<ServiceResponse, ServiceResponse> {
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
        None => return new_unauthorized_response!(""),
    };

    //  we have to embed the TestContext in the claim because we come through a GET from a URL where
    //  we can't add headers.
    let request_context = RequestContext::new(
        &Some(claims.clone()),
        &claims.test_context,
        &SERVICE_CONFIG,
        &security_context,
    );

    let id = &claims.id; // Borrowing here.
    let mut user = request_context.database.find_user_by_id(id).await?;

    user.user_profile.validated_email = true;
    request_context.database.update_or_create_user(&user).await
}

//
//  url is in the form of host://api/v1/users/validate-email/<token>
pub fn get_validation_url(
    host: &str,
    id: &str,
    email: &str,
    request_context: &RequestContext,
) -> String {
    let claims = Claims::new(
        id,
        email,
        60 * 10,
        &vec![Role::Validation],
        &request_context.test_context,
    ); // 10 minutes
    let token = request_context
        .security_context
        .validation_keys
        .sign_claims(&claims)
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
pub fn send_validation_email(
    request_context: &RequestContext,
) -> Result<ServiceResponse, ServiceResponse> {
    trace_function!("send_validation_email");
    let host_name = std::env::var("HOST_NAME").expect("HOST_NAME must be set");
    let claims = request_context
        .claims
        .clone()
        .expect("claims are set by auth middleware, or the call is rejected");
    let url = get_validation_url(&host_name, &claims.id, &claims.sub, &request_context);
    let msg = format!(
        "Thank you for registering with our Service.\n\n\
         Click on this link to validate your email: {}\n\n\
         If you did not register with the service, something has gone terribly wrong.",
        url
    );
    let result = send_email(
        &claims.sub,
        &SERVICE_CONFIG.service_email,
        "Please validate your email",
        &msg,
    );
    match result {
        Ok(_) => Ok(ServiceResponse::new(
            "sent",
            StatusCode::OK,
            ResponseType::Url(url),
            GameError::NoError(String::default()),
        )),
        Err(e) => Err(ServiceResponse::new(
            "Error sending email",
            reqwest::StatusCode::INTERNAL_SERVER_ERROR,
            ResponseType::ErrorInfo(e),
            GameError::HttpError(StatusCode::INTERNAL_SERVER_ERROR),
        )),
    }
}
///
/// 1. get the user profile
/// 2. generate a random 6 digit number
/// 3. store the number in the profile
/// 4. update the profile
/// 5. send the text message to the phone
pub async fn send_phone_code(
    request_context: &RequestContext,
) -> Result<ServiceResponse, ServiceResponse> {
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
) -> Result<ServiceResponse, ServiceResponse> {
    let mut persist_user = request_context.database.find_user_by_id(user_id).await?;

    let phone_number = match &persist_user.user_profile.pii {
        Some(pii) => pii.phone_number.clone(),
        None => return Err(bad_request_from_string!("no phone number in profile")),
    };

    persist_user.phone_code = Some(code.to_string());
    request_context
        .database
        .update_or_create_user(&persist_user)
        .await?;
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
) -> Result<ServiceResponse, ServiceResponse> {
    let user_id = request_context
        .claims
        .as_ref()
        .expect("auth_mw should have added this or rejected the call")
        .id
        .clone();

    let mut persist_user = request_context.database.find_user_by_id(&user_id).await?;

    match &persist_user.phone_code {
        // If the stored code matches the provided code, validate the phone.
        Some(c) if c.to_string() == code => {
            persist_user.user_profile.validated_phone = true;
            persist_user.phone_code = None;
            request_context
                .database
                .update_or_create_user(&persist_user)
                .await?;
            Ok(ServiceResponse::new(
                "validated",
                StatusCode::OK,
                ResponseType::NoData,
                GameError::NoError(String::default()),
            ))
        }
        // Handle all other cases (mismatch or missing code) as errors.
        _ => Err(ServiceResponse::new(
            "incorrect code.  request a new one",
            reqwest::StatusCode::BAD_REQUEST,
            ResponseType::NoData,
            GameError::HttpError(StatusCode::BAD_REQUEST),
        )),
    }
}

///
/// rotates the login keys -- admin only
///

pub async fn rotate_login_keys(
    request_context: &RequestContext,
) -> Result<ServiceResponse, ServiceResponse> {
    panic!("update this to match the new SecurityContext naming convention");
    // if !request_context.is_caller_in_role(Role::Admin) {
    //     return new_unauthorized_response!("");
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

//
//  this is used in many places internally and has to work for all callers. - todo: make internal
//  note: the handler is locked down to admin or Self
pub async fn find_user_by_id(
    id: &str,
    request_context: &RequestContext,
) -> Result<ServiceResponse, ServiceResponse> {
    

    let persist_user = request_context.database.find_user_by_id(&id).await?;

    Ok(ServiceResponse::new(
        "",
        StatusCode::OK,
        ResponseType::Profile(UserProfile::from_persist_user(&persist_user)),
        GameError::NoError(String::default()),
    ))
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
    profile_in: &mut UserProfile,
    request_context: &RequestContext,
) -> Result<ServiceResponse, ServiceResponse> {
    let user_id = request_context
        .claims
        .as_ref()
        .expect("auth_mw should have added this or rejected the call")
        .id
        .clone();

    profile_in.pii = None;
    profile_in.user_type = UserType::Local;
    profile_in.games_played = Some(0);
    profile_in.games_won = Some(0);
    //
    //  create a hash that is extremely unlikely to be guessed so that the user can't login

    let persist_user = PersistUser::from_local_user(&user_id, &profile_in);

    request_context
        .database
        .update_or_create_user(&persist_user)
        .await
}

///
/// only an admin or the ConnectedUser can update the LocalUser
pub async fn update_local_user(
    id: &str,
    request_context: &RequestContext,
) -> Result<ServiceResponse, ServiceResponse> {
    let user_id = request_context
        .claims
        .as_ref()
        .expect("auth_mw should have added this or rejected the call")
        .id
        .clone();

    let user = find_user_by_id(&user_id, &request_context).await?;

    if user_id != id && !request_context.is_caller_in_role(Role::Admin) {
        return new_unauthorized_response!("only an admin can delete another user");
    }

    todo!()
}
///
/// only an admin or the ConnectedUser can delete the LocalUser
pub async fn delete_local_user(
    request_context: &RequestContext,
) -> Result<ServiceResponse, ServiceResponse> {
    todo!()
}
/// to get the full list of local users we need to
/// 1. get the PersistProfile of the caller
/// 2. use the local_user_owner_id to do a query against request_context.database.get_local_users()
pub async fn get_local_users(
    request_context: &RequestContext,
) -> Result<ServiceResponse, ServiceResponse> {
    todo!()
}
#[cfg(test)]
mod tests {

    use crate::{init_env_logger, test::test_helpers::test::TestHelpers};

    use super::*;

    // Test the login function
    #[tokio::test]
    async fn test_login_mocked() {
        init_env_logger(log::LevelFilter::Trace, log::LevelFilter::Trace).await;
        test_login(false).await;
    }

    #[tokio::test]
    async fn test_login_cosmos() {
        init_env_logger(log::LevelFilter::Info, log::LevelFilter::Error).await;
        test_login(true).await;
    }

    async fn test_login(use_cosmos: bool) {
        let token = TestHelpers::admin_login().await;

        let request_context = RequestContext::test_default(use_cosmos);
        let profile = UserProfile::new_test_user(None);
        // setup
        // let response = verify_cosmosdb(&request_context).await;
        // assert!(response.is_ok());
        // Register the user first
        let result = register("password", &profile, &request_context).await;
        assert!(result.is_ok());
        let sr = result.unwrap();
        let user_profile = sr.to_profile().expect("This should be a client user");
        request_context
            .database
            .find_user_by_email(&user_profile.pii.unwrap().email)
            .await
            .expect("we just put this here!");

        // Test login with correct credentials
        let response = login(&profile.get_email_or_panic(), "password", &request_context)
            .await
            .expect("login should succeed");

        // Test login with incorrect credentials
        let response = login(
            &profile.get_email_or_panic(),
            "wrong_password",
            &request_context,
        )
        .await;
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
            &None,
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
