#![allow(dead_code)]
#![allow(unused_variables)]

use bcrypt::{hash, verify};
use rand::Rng;
use url::form_urlencoded;

use crate::azure_setup::azure_wrapper::{
    cosmos_account_exists, cosmos_collection_exists, cosmos_database_exists, key_vault_get_secret,
    key_vault_save_secret, keyvault_exists, send_email, send_text_message, verify_login_or_panic,
};
use crate::middleware::security_context::{ KeyKind, SecurityContext};
use crate::middleware::service_config::SERVICE_CONFIG;
use crate::shared::service_models::{Claims, PersistUser, Role};
/**
 * this module implements the WebApi to create the database/collection, list all the users, and to create/find/delete
 * a User document in CosmosDb
 */
use crate::{ new_ok_response, new_unauthorized_response, trace_function};

use crate::games_service::long_poller::long_poller::LongPoller;

use crate::middleware::request_context_mw::RequestContext;
use crate::shared::shared_models::{
    ClientUser, GameError, ResponseType, ServiceResponse, UserProfile,
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
    if request_context
        .database
        .find_user_by_email(&profile_in.email)
        .await
        .is_ok() // you can't register twice!
    {
        return Err(ServiceResponse::new(
            "User already exists",
            StatusCode::CONFLICT,
            ResponseType::NoData,
            GameError::HttpError(StatusCode::CONFLICT),
        ));
    }
    // this lets us bootstrap the system -- my assumption is that if you can set the environment variable, then you are
    // an admin.
    if profile_in.email == request_context.config.admin_email {
        if request_context.is_test() {
            return Err(ServiceResponse::new(
                "Should not have test context",
                StatusCode::BAD_REQUEST,
                ResponseType::NoData,
                GameError::HttpError(StatusCode::BAD_REQUEST),
            ));
        }
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
    internal_register_user(password, profile_in, &mut vec![Role::User], request_context).await
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
    let user = match request_context
        .database
        .find_user_by_email(username)
        .await?
    {
        Some(u) => u,
        None => {
            return new_unauthorized_response!("");
        }
    };

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
            &user.user_profile.email,
            24 * 60 * 60,
            &user.roles,
            &request_context.test_context,
        );
        let token_result = request_context.security_context.login_keys.sign_claims(&claims);
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
            let client_users: Vec<ClientUser> = users
                .iter()
                .map(|user| ClientUser::from_persist_user(&user))
                .collect();

            Ok(ServiceResponse::new(
                "",
                StatusCode::OK,
                ResponseType::ClientUsers(client_users),
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
pub async fn get_profile(
    request_context: &RequestContext,
) -> Result<ServiceResponse, ServiceResponse> {
    let user_id = request_context
        .claims
        .as_ref()
        .expect("auth_mw should have added this or rejected the call")
        .id
        .clone();
    let user = match request_context.database.find_user_by_id(&user_id).await? {
        Some(u) => u,
        None => {
            return Ok(ServiceResponse {
                message: format!("id {} not found", user_id),
                status: StatusCode::NOT_FOUND,
                response_type: ResponseType::NoData,
                game_error: GameError::NoError(String::default()),
            });
        }
    };
    Ok(ServiceResponse::new(
        "",
        StatusCode::OK,
        ResponseType::ClientUser(ClientUser::from_persist_user(&user)),
        GameError::NoError(String::default()),
    ))
}

pub async fn delete(request_context: &RequestContext) -> Result<ServiceResponse, ServiceResponse> {
    let user_id = request_context
        .claims
        .as_ref()
        .expect("auth_mw should have added this or rejected the call")
        .id
        .clone();

    let result = request_context.database.delete_user(&user_id).await;

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
    let claims = match security_context.validation_keys.validate_token(&decoded_token) {
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
    let mut user = match request_context.database.find_user_by_id(id).await? {
        Some(u) => u,
        None => {
            return Ok(ServiceResponse {
                message: format!("id {} not found", id),
                status: StatusCode::NOT_FOUND,
                response_type: ResponseType::NoData,
                game_error: GameError::NoError(String::default()),
            });
        }
    };

    user.validated_email = true;
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
    host: &str,
    id: &str,
    email: &str,
    request_context: &RequestContext,
) -> Result<ServiceResponse, ServiceResponse> {
    trace_function!("send_validation_email");
    let url = get_validation_url(host, id, email, &request_context);
    let msg = format!(
        "Thank you for registering with our Service.\n\n\
         Click on this link to validate your email: {}\n\n\
         If you did not register with the service, something has gone terribly wrong.",
        url
    );
    let result = send_email(
        email,
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
    user_id: &str,
    request_context: &RequestContext,
) -> Result<ServiceResponse, ServiceResponse> {
    let code = rand::thread_rng().gen_range(100_000..=999_999);
    internal_send_phone_code(user_id, code, request_context).await
}
///
/// I have the send_phone_code() and the internal_send_phone_code() so that i can test internal_send_phone_code() in an
/// automated way (by having the test create the code and pass it in and then pass it to validate_phone())
async fn internal_send_phone_code(
    user_id: &str,
    code: i32,
    request_context: &RequestContext,
) -> Result<ServiceResponse, ServiceResponse> {
    let mut persist_user = match request_context.database.find_user_by_id(user_id).await? {
        Some(u) => u,
        None => {
            return Ok(ServiceResponse {
                message: format!("id {} not found", user_id),
                status: StatusCode::NOT_FOUND,
                response_type: ResponseType::NoData,
                game_error: GameError::NoError(String::default()),
            });
        }
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
    send_text_message(&persist_user.user_profile.phone_number, &msg)
}

/// Validates a phone code for a given user.
///
/// This function checks if the provided phone code matches the stored code for the user.
/// If the code matches, it updates the user's information to indicate a validated phone.
///
/// # Arguments
///
/// * `user_id` - The unique identifier for the user.
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

    let mut persist_user = match request_context.database.find_user_by_id(&user_id).await? {
        Some(u) => u,
        None => {
            return Ok(ServiceResponse {
                message: format!("id {} not found", user_id),
                status: StatusCode::NOT_FOUND,
                response_type: ResponseType::NoData,
                game_error: GameError::NoError(String::default()),
            });
        }
    };

    match &persist_user.phone_code {
        // If the stored code matches the provided code, validate the phone.
        Some(c) if c.to_string() == code => {
            persist_user.validated_phone = true;
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
    if !request_context.is_caller_in_role(Role::Admin) {
        return new_unauthorized_response!("");
    }

    let kv_name = request_context.config.kv_name.to_owned();
    let old_name = "oldLoginSecret-test";
    let new_name = "currentLoginSecret-test";

    // make sure the user/service is logged in
    verify_login_or_panic();

    // verify KV exists
    keyvault_exists(&kv_name).expect(&format!("Failed to find Key Vault named {}.", kv_name));

    let current_primary_login_key = key_vault_get_secret(&kv_name, KeyKind::PRIMARY_KEY)?;

    let new_key = SecurityContext::generate_jwt_key();

    key_vault_save_secret(&kv_name, KeyKind::SECONDARY_KEY, &current_primary_login_key)?;
    key_vault_save_secret(&kv_name, KeyKind::PRIMARY_KEY, &new_key)?;

    return new_ok_response!("");
}

#[cfg(test)]
mod tests {
    use actix_web::test;

    use crate::{
        create_test_service, games_service::game_container::game_messages::GameHeader,
        init_env_logger, middleware::service_config::SERVICE_CONFIG, setup_test, test::test_helpers::test::TestHelpers,
    };

    use super::*;
    #[tokio::test]
    async fn test_validate_phone() {
        init_env_logger(log::LevelFilter::Info, log::LevelFilter::Error).await;
        let app = create_test_service!();
        setup_test!(&app, false);

        let request_context = RequestContext::test_default(false);
        let mut profile = UserProfile::new_test_user();
        profile.phone_number = SERVICE_CONFIG.test_phone_number.clone();
        // setup
        let response = verify_cosmosdb(&request_context).await;

        let sr = register("password", &profile, &request_context)
            .await
            .expect("this should work");
        let client_user = sr.get_client_user().expect("This should be a client user");
        let code = 12345;
        let sr = internal_send_phone_code(&client_user.id, code, &request_context)
            .await
            .expect("text message should be sent!");

        log::trace!("code is: {}", code);
        let sr = validate_phone(&format!("{}", code), &request_context)
            .await
            .expect("validation to work");
        assert_eq!(sr.status, StatusCode::OK);
    }

    #[tokio::test]
    async fn test_validate_email() {
        init_env_logger(log::LevelFilter::Trace, log::LevelFilter::Error).await;
        let app = create_test_service!();
        setup_test!(&app, false);

        let request_context = RequestContext::test_default(false);
        let mut profile = UserProfile::new_test_user();
        profile.email = SERVICE_CONFIG.test_email.clone();
        // setup
        let response = verify_cosmosdb(&request_context).await;

        let sr = register("password", &profile, &request_context)
            .await
            .expect("this should work");
        let client_user = sr.get_client_user().expect("This should be a client user");
        let host_name = std::env::var("HOST_NAME").expect("HOST_NAME must be set");
        let result = send_validation_email(
            &host_name,
            &client_user.id,
            &profile.email,
            &request_context,
        );
        match result {
            Ok(service_response) => {
                let url = service_response.get_url().expect("this should be a URL!");
                let req = test::TestRequest::get().uri(&url).to_request();
                let resp = test::call_service(&app, req).await;
                assert!(resp.status().is_success());
            }
            Err(sr) => {
                log::error!("{}", sr);
                panic!("should not have failed to send an email");
            }
        }
    }

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
        let profile = UserProfile::new_test_user();
        // setup
        // let response = verify_cosmosdb(&request_context).await;
        // assert!(response.is_ok());
        // Register the user first
        let result = register("password", &profile, &request_context).await;
        assert!(result.is_ok());
        let sr = result.unwrap();
        let client_user = sr.get_client_user().expect("This should be a client user");
        request_context
            .database
            .find_user_by_email(&client_user.user_profile.email)
            .await
            .expect("we just put this here!");

        // Test login with correct credentials
        let response = login(&profile.email, "password", &request_context)
            .await
            .expect("login should succeed");

        // Test login with incorrect credentials
        let response = login(&profile.email, "wrong_password", &request_context).await;
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
        let token = request_context.security_context.login_keys.sign_claims(&claims).unwrap();
        let token_claims = request_context.security_context.login_keys.validate_token(&token).unwrap();
        assert_eq!(token_claims, claims);
    }
}
