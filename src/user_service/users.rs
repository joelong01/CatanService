#![allow(dead_code)]
#![allow(unused_variables)]
use rand::RngCore;
use std::sync::atomic::{AtomicBool, Ordering};
use url::form_urlencoded;

use crate::azure_setup::azure_wrapper::{
    cosmos_account_exists, cosmos_collection_exists, cosmos_database_exists, send_email,
};
/**
 * this module implements the WebApi to create the database/collection, list all the users, and to create/find/delete
 * a User document in CosmosDb
 */
use crate::cosmos_db::cosmosdb::UserDb;
use crate::cosmos_db::mocked_db::TestDb;
use crate::{full_info, trace_function};

use crate::games_service::long_poller::long_poller::LongPoller;

use crate::middleware::environment_mw::{RequestContext, TestContext, CATAN_ENV};
use crate::shared::models::{
    Claims, ClientUser, GameError, PersistUser, ResponseType, ServiceResponse, UserProfile,
};

use bcrypt::{hash, verify};
use jsonwebtoken::{
    decode, encode, Algorithm, DecodingKey, EncodingKey, Header, TokenData, Validation,
};
use lazy_static::lazy_static;
use reqwest::StatusCode;
use tokio::sync::Mutex;

lazy_static! {
    static ref DB_SETUP: AtomicBool = AtomicBool::new(false);
    static ref SETUP_LOCK: Mutex<()> = Mutex::new(());
}
/**
 * this sets up CosmosDb to make the sample run. the only prereq is the secrets set in
 * .devconainter/required-secrets.json, this API will call setupdb. this just calls the setupdb api and deals with errors
 *
 * you can't do the normal authn/authz here because the authn path requires the database to exist.  for this app,
 * the users database will be created out of band and this path is just for test users.
 */

pub async fn setup(context: &RequestContext) -> Result<ServiceResponse, ServiceResponse> {
    trace_function!("setup");
    let use_cosmos_db = match &context.test_context {
        Some(tc) => tc.use_cosmos_db,
        None => {
            return Err(ServiceResponse::new(
                "Test Header must be set",
                StatusCode::UNAUTHORIZED,
                ResponseType::NoData,
                GameError::HttpError,
            ))
        }
    };

    if use_cosmos_db {
        if cosmos_account_exists(&context.env.cosmos_account, &context.env.resource_group).is_err()
        {
            return Err(ServiceResponse::new(
                &format!("account {} does not exist", context.env.cosmos_account),
                StatusCode::NOT_FOUND,
                ResponseType::NoData,
                GameError::HttpError,
            ));
        }

        if cosmos_database_exists(
            &context.env.cosmos_account,
            &context.env.cosmos_database,
            &context.env.resource_group,
        )
        .is_err()
        {
            return Err(ServiceResponse::new(
                &format!(
                    "account {} does exists, but database {} does not",
                    context.env.cosmos_account, context.env.cosmos_database
                ),
                StatusCode::NOT_FOUND,
                ResponseType::NoData,
                GameError::HttpError,
            ));
        }
        for collection in &context.env.cosmos_collections {
            let collection_exists = cosmos_collection_exists(
                &context.env.cosmos_account,
                &context.env.cosmos_database,
                &collection,
                &context.env.resource_group,
            );

            if collection_exists.is_err() {
                let error_message = format!(
                    "account {} exists, database {} exists, but {} does not",
                    context.env.cosmos_account, context.env.cosmos_database, collection
                );

                return Err(ServiceResponse::new(
                    &error_message,
                    StatusCode::NOT_FOUND,
                    ResponseType::NoData,
                    GameError::HttpError,
                ));
            }
        }

        return Ok(ServiceResponse::new(
            "already exists",
            StatusCode::ACCEPTED,
            ResponseType::NoData,
            GameError::NoError,
        ));
    }

    if DB_SETUP.load(Ordering::Relaxed) {
        return Ok(ServiceResponse::new(
            "already exists",
            StatusCode::ACCEPTED,
            ResponseType::NoData,
            GameError::NoError,
        ));
    }

    let _lock_guard = SETUP_LOCK.lock().await;

    if DB_SETUP.load(Ordering::Relaxed) {
        return Ok(ServiceResponse::new(
            "already exists",
            StatusCode::ACCEPTED,
            ResponseType::NoData,
            GameError::NoError,
        ));
    }

    match TestDb::setupdb().await {
        Ok(..) => {
            DB_SETUP.store(true, Ordering::Relaxed);
            return Ok(ServiceResponse::new(
                "created",
                StatusCode::CREATED,
                ResponseType::NoData,
                GameError::NoError,
            ));
        }
        Err(e) => Err(ServiceResponse::new(
            "Bad Request",
            StatusCode::BAD_REQUEST,
            ResponseType::ErrorInfo(format!("{:#?}", e)),
            GameError::HttpError,
        )),
    }
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
    if internal_find_user("Email", &profile_in.email, request_context)
        .await
        .is_ok()
    {
        return Err(ServiceResponse::new(
            "User already exists",
            StatusCode::CONFLICT,
            ResponseType::NoData,
            GameError::HttpError,
        ));
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
                GameError::HttpError,
            ));
        }
    };

    // Create the user record
    let mut persist_user = PersistUser::from_user_profile(&profile_in, password_hash.to_owned());

    // ignore the game stats passed in by the client and create a new one
    persist_user.user_profile.games_played = Some(0);
    persist_user.user_profile.games_won = Some(0);
    internal_update_user(&persist_user, &request_context).await
}

async fn internal_update_user(
    persist_user: &PersistUser,
    request_context: &RequestContext,
) -> Result<ServiceResponse, ServiceResponse> {
    // Create the database connection
    if request_context.use_cosmos_db() {
        let userdb = UserDb::new(&request_context).await;

        // Save the user record to the database
        match userdb.update_or_create_user(persist_user.clone()).await {
            Ok(..) => Ok(ServiceResponse::new(
                "created",
                StatusCode::CREATED,
                ResponseType::ClientUser(ClientUser::from_persist_user(persist_user)),
                GameError::NoError,
            )),
            Err(e) => {
                return Err(ServiceResponse::new(
                    "Bad Request",
                    StatusCode::BAD_REQUEST,
                    ResponseType::ErrorInfo(format!("{:#?}", e)),
                    GameError::HttpError,
                ))
            }
        }
    } else {
        TestDb::update_or_create_user(&persist_user)
            .await
            .unwrap();
        Ok(ServiceResponse::new(
            "created",
            StatusCode::CREATED,
            ResponseType::ClientUser(ClientUser::from_persist_user(persist_user)),
            GameError::NoError,
        ))
    }
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
    let user = internal_find_user("Email", username, request_context).await?;
    let password_hash: String = match user.password_hash {
        Some(p) => p,
        None => {
            return Err(ServiceResponse::new(
                "user document does not contain a password hash",
                StatusCode::INTERNAL_SERVER_ERROR,
                ResponseType::NoData,
                GameError::HttpError,
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
                GameError::HttpError,
            ));
        }
    };

    if is_password_match {
        let claims = Claims::new(
            &user.id,
            &user.user_profile.email,
            24 * 60 * 60,
            &request_context.test_context,
        );
        let token_result = create_jwt_token(&claims, &request_context.env.login_secret_key);
        match token_result {
            Ok(token) => {
                let _ = LongPoller::add_user(&user.id, &user.user_profile).await;
                Ok(ServiceResponse::new(
                    "",
                    StatusCode::OK,
                    ResponseType::Token(token),
                    GameError::NoError,
                ))
            }
            Err(e) => {
                return Err(ServiceResponse::new(
                    "Error Hashing token",
                    StatusCode::INTERNAL_SERVER_ERROR,
                    ResponseType::ErrorInfo(format!("{:#?}", e)),
                    GameError::HttpError,
                ));
            }
        }
    } else {
        return Err(ServiceResponse::new(
            "invalid password",
            StatusCode::UNAUTHORIZED,
            ResponseType::NoData,
            GameError::HttpError,
        ));
    }
}

pub fn generate_jwt_key() -> String {
    let mut key = [0u8; 96]; // 96 bytes * 8 bits/byte = 768 bits.
    rand::thread_rng().fill_bytes(&mut key);
    openssl::base64::encode_block(&key)
}

pub fn create_jwt_token(
    claims: &Claims,
    secret_key: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let token_result = encode(
        &Header::new(Algorithm::HS512),
        &claims,
        &EncodingKey::from_secret(secret_key.as_ref()),
    );

    token_result.map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
}
pub fn validate_jwt_token(token: &str, secret_key: &str) -> Option<TokenData<Claims>> {
    let validation = Validation::new(Algorithm::HS512);

    match decode::<Claims>(
        &token,
        &DecodingKey::from_secret(secret_key.as_ref()),
        &validation,
    ) {
        Ok(c) => {
            Some(c) // or however you want to handle a valid token
        }
        Err(e) => {
            full_info!("token NOT VALID: {:?}", e);
            None
        }
    }
}

/**
 *  this will get a list of all documents.  Note this does *not* do pagination. This would be a reasonable next step to
 *  show in the sample
 */
pub async fn list_users(
    request_context: &RequestContext,
) -> Result<ServiceResponse, ServiceResponse> {
    if request_context.use_cosmos_db() {
        let userdb = UserDb::new(&request_context).await;

        // Get list of users
        match userdb.list().await {
            Ok(users) => {
                let client_users: Vec<ClientUser> = users
                    .iter()
                    .map(|user| ClientUser::from_persist_user(&user))
                    .collect();

                Ok(ServiceResponse::new(
                    "",
                    StatusCode::OK,
                    ResponseType::ClientUsers(client_users),
                    GameError::NoError,
                ))
            }
            Err(err) => {
                return Err(ServiceResponse::new(
                    "",
                    StatusCode::NOT_FOUND,
                    ResponseType::ErrorInfo(format!("Failed to retrieve user list: {}", err)),
                    GameError::HttpError,
                ));
            }
        }
    } else {
        let client_users: Vec<ClientUser> = TestDb::list()
            .await
            .unwrap()
            .iter()
            .map(|user| ClientUser::from_persist_user(&user))
            .collect();

        Ok(ServiceResponse::new(
            "",
            StatusCode::OK,
            ResponseType::ClientUsers(client_users),
            GameError::NoError,
        ))
    }
}
pub async fn get_profile(
    user_id: &str,
    request_context: &RequestContext,
) -> Result<ServiceResponse, ServiceResponse> {
    let user = internal_find_user("id", user_id, request_context).await?;

    Ok(ServiceResponse::new(
        "",
        StatusCode::OK,
        ResponseType::ClientUser(ClientUser::from_persist_user(&user)),
        GameError::NoError,
    ))
}

pub async fn find_user_by_id(
    id: &str,
    request_context: &RequestContext,
) -> Result<ServiceResponse, ServiceResponse> {
    get_profile(id, request_context).await
}

pub async fn internal_find_user(
    prop: &str,
    val: &str,
    request_context: &RequestContext,
) -> Result<PersistUser, ServiceResponse> {
    if request_context.use_mock_db() {
        if prop == "id" {
            return TestDb::find_user_by_id(val).await;
        } else {
            return TestDb::find_user_by_email(val).await;
        }
    }

    let userdb = UserDb::new(request_context).await;
    if prop == "id" {
        userdb.find_user_by_id(val).await
    } else {
        userdb.find_user_by_email(val).await
    }
}

pub async fn delete(
    id_path: &str,
    id_token: &str,
    request_context: &RequestContext,
) -> Result<ServiceResponse, ServiceResponse> {
    //
    // unwrap is ok here because our middleware put it there.

    if id_path != id_token {
        return Err(ServiceResponse::new(
            "you can only delete yourself",
            StatusCode::UNAUTHORIZED,
            ResponseType::NoData,
            GameError::HttpError,
        ));
    }

    let result;
    if request_context.use_cosmos_db() {
        let userdb = UserDb::new(&request_context).await;
        result = userdb.delete_user(&id_path).await
    } else {
        result = TestDb::delete_user(&id_path).await;
    }
    match result {
        Ok(..) => Ok(ServiceResponse::new(
            &format!("deleted user with id: {}", id_path),
            StatusCode::OK,
            ResponseType::NoData,
            GameError::NoError,
        )),
        Err(err) => {
            return Err(ServiceResponse::new(
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
pub async fn validate_email(
    token: &str,
    request_context: &RequestContext,
) -> Result<ServiceResponse, ServiceResponse> {
    trace_function!("validate_email");
    let decoded_token = form_urlencoded::parse(token.as_bytes())
        .map(|(key, _)| key)
        .collect::<Vec<_>>()
        .join("");

    if let Some(claims) =
        validate_jwt_token(&decoded_token, &request_context.env.validation_secret_key)
    {
        // Extract the id and sub from the claims
        let id = &claims.claims.id;
        let email = &claims.claims.sub;
        //
        //  we have to embed the TestContext in the claim because we come through a GET from a URL where
        //  we can't add headers.
        let mut ctx = request_context.clone();
        ctx.test_context = claims.claims.test_context;
        let mut user = internal_find_user("id", &claims.claims.id, &ctx).await?;
        user.validated_email = true;
        return internal_update_user(&user, &ctx).await;
    } else {
        return Err(ServiceResponse::new(
            "unauthorized",
            StatusCode::UNAUTHORIZED,
            ResponseType::NoData,
            GameError::HttpError,
        ));
    }
}
//
//  url is in the form of host://api/v1/users/validate_email/<token>
pub fn get_validation_url(
    host: &str,
    id: &str,
    email: &str,
    test_context: &Option<TestContext>,
) -> String {
    let claims = Claims::new(id, email, 60 * 10, test_context); // 10 minutes
    let token = create_jwt_token(&claims, &CATAN_ENV.validation_secret_key)
        .expect("Token creation should not fail");

    let encoded_token = form_urlencoded::byte_serialize(token.as_bytes()).collect::<String>();

    format!(
        "https://{}/api/v1/users/validate_email/{}",
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
    let url = get_validation_url(host, id, email, &request_context.test_context);
    let msg = format!(
        "Thank you for registering with our Service.\n\n\
         Click on this link to validate your email: {}\n\n\
         If you did not register with the service, something has gone terribly wrong.",
        url
    );
    let result = send_email(
        email,
        &CATAN_ENV.service_email,
        "Please validate your email",
        &msg,
    );
    match result {
        Ok(_) => Ok(ServiceResponse::new(
            "sent",
            StatusCode::OK,
            ResponseType::Url(url),
            GameError::NoError,
        )),
        Err(e) => Err(ServiceResponse::new(
            "Error sending email",
            reqwest::StatusCode::INTERNAL_SERVER_ERROR,
            ResponseType::ErrorInfo(e),
            GameError::HttpError,
        )),
    }
}

#[cfg(test)]
mod tests {
    use actix_web::test;
 

    use crate::{
        create_test_service, games_service::game_container::game_messages::GameHeader,
        init_env_logger, middleware::environment_mw::CATAN_ENV, setup_test,
    };

    use super::*;
    #[tokio::test]
    async fn test_validate_email() {
        init_env_logger(log::LevelFilter::Trace, log::LevelFilter::Error).await;
        let app = create_test_service!();
        setup_test!(&app, false);

        let request_context = RequestContext::test_default(false);
        let mut profile = UserProfile::new_test_user();
        profile.email = CATAN_ENV.test_email.clone();
        // setup
        let response = setup(&request_context).await;
       
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
    async fn test_login_mocked_and_cosmos() {
        test_login(true).await;
        test_login(false).await;
    }

    async fn test_login(use_cosmos: bool) {
        let request_context = RequestContext::test_default(use_cosmos);
        let profile = UserProfile::new_test_user();
        // setup
        let response = setup(&request_context).await;

        // Register the user first
        let sr = register("password", &profile, &request_context)
            .await
            .expect("this should work");
        let client_user = sr.get_client_user().expect("This should be a client user");

        let user = internal_find_user("email", &client_user.user_profile.email, &request_context)
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
        let claims = Claims::new("user_id", "user_email@email.com", 10 * 60, &None);
        let token = create_jwt_token(&claims, &CATAN_ENV.login_secret_key).unwrap();
        let token_claims = validate_jwt_token(&token, &CATAN_ENV.login_secret_key);
        assert!(token_claims.is_some());
        let token_claims = token_claims.expect("there should be a token in here");
        assert_eq!(token_claims.claims, claims);
    }
}
