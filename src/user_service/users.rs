#![allow(dead_code)]
#![allow(unused_variables)]
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/**
 * this module implements the WebApi to create the database/collection, list all the users, and to create/find/delete
 * a User document in CosmosDb
 */
use crate::cosmos_db::cosmosdb::UserDb;
use crate::cosmos_db::mocked_db::TestDb;
use crate::full_info;

use crate::games_service::long_poller::long_poller::LongPoller;

use crate::middleware::environment_mw::{ServiceEnvironmentContext, CATAN_ENV};
use crate::shared::models::{
    Claims, ClientUser, GameError, PersistUser, ServiceResponse, UserProfile,
};
use actix_web::web::Data;
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

pub async fn setup(
    is_test: bool,
    database_name: &str,
    container_name: &str,
) -> Result<ServiceResponse<String>, ServiceResponse<String>> {
    if !is_test {
        return Err(ServiceResponse::new(
            "Test Header must be set",
            StatusCode::UNAUTHORIZED,
            String::new(),
            GameError::HttpError,
        ));
    }
    if DB_SETUP.load(Ordering::Relaxed) {
        return Ok(ServiceResponse::new(
            "already exists",
            StatusCode::ACCEPTED,
            String::new(),
            GameError::NoError,
        ));
    }

    let _lock_guard = SETUP_LOCK.lock().await;

    if DB_SETUP.load(Ordering::Relaxed) {
        return Ok(ServiceResponse::new(
            "already exists",
            StatusCode::ACCEPTED,
            String::new(),
            GameError::NoError,
        ));
    }

    match TestDb::setupdb().await {
        Ok(..) => {
            DB_SETUP.store(true, Ordering::Relaxed);
            return Ok(ServiceResponse::new(
                "created",
                StatusCode::CREATED,
                String::new(),
                GameError::NoError,
            ));
        }
        Err(e) => Err(ServiceResponse::new(
            "Bad Request",
            StatusCode::BAD_REQUEST,
            format!("{:#?}", e),
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
    is_test: bool,
    password: &str,
    profile_in: &UserProfile,
    data: &Data<ServiceEnvironmentContext>,
) -> Result<ServiceResponse<ClientUser>, ServiceResponse<String>> {
    if internal_find_user("email", &profile_in.email, is_test, data)
        .await
        .is_ok()
    {
        return Err(ServiceResponse::new(
            "User already exists",
            StatusCode::CONFLICT,
            String::new(),
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
                err_message.to_owned(),
                GameError::HttpError,
            ));
        }
    };

    // Create the user record
    let mut persist_user = PersistUser::from_user_profile(&profile_in, password_hash.to_owned());

    // ignore the game stats passed in by the client and create a new one
    persist_user.user_profile.games_played = Some(0);
    persist_user.user_profile.games_won = Some(0);
    // Create the database connection
    if !is_test {
        let request_context = data.context.lock().unwrap();
        let userdb = UserDb::new(&request_context).await;

        // Save the user record to the database
        match userdb.create_user(persist_user.clone()).await {
            Ok(..) => {
                persist_user.password_hash = None;
                Ok(ServiceResponse::new(
                    "created",
                    StatusCode::CREATED,
                    ClientUser::from_persist_user(persist_user),
                    GameError::NoError,
                ))
            }
            Err(e) => {
                return Err(ServiceResponse::new(
                    "Bad Request",
                    StatusCode::BAD_REQUEST,
                    format!("{:#?}", e),
                    GameError::HttpError,
                ))
            }
        }
    } else {
        //
        //  if it is a test, set the user_id to the email name so that it is easier to follow in the logs
        persist_user.id = profile_in.email.clone();
        TestDb::create_user(persist_user.clone()).await.unwrap();
        Ok(ServiceResponse::new(
            "created",
            StatusCode::CREATED,
            ClientUser::from_persist_user(persist_user),
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
    is_test: bool,
    data: &Data<ServiceEnvironmentContext>,
) -> Result<ServiceResponse<String>, ServiceResponse<String>> {
    let user_result = internal_find_user("email", username, is_test, data).await;

    let user = match user_result {
        Ok(u) => u,
        Err(e) => {
            return Err(ServiceResponse::new(
                &format!("invalid user id: {}", username),
                StatusCode::NOT_FOUND,
                format!("{:#?}", e),
                GameError::HttpError,
            ))
        }
    };
    let password_hash: String = match user.password_hash {
        Some(p) => p,
        None => {
            return Err(ServiceResponse::new(
                "user document does not contain a password hash",
                StatusCode::INTERNAL_SERVER_ERROR,
                String::new(),
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
                String::new(),
                GameError::HttpError,
            ));
        }
    };

    if is_password_match {
        let token_result = create_jwt_token(&user.id, &user.user_profile.email);
        match token_result {
            Ok(token) => {
                let _ = LongPoller::add_user(&user.id, &user.user_profile).await;
                Ok(ServiceResponse::new(
                    "",
                    StatusCode::OK,
                    token,
                    GameError::NoError,
                ))
            }
            Err(e) => {
                return Err(ServiceResponse::new(
                    "Error Hashing token",
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("{:#?}", e),
                    GameError::HttpError,
                ));
            }
        }
    } else {
        return Err(ServiceResponse::new(
            "invalid password",
            StatusCode::UNAUTHORIZED,
            String::new(),
            GameError::HttpError,
        ));
    }
}

fn create_jwt_token(id: &str, email: &str) -> Result<String, Box<dyn std::error::Error>> {
    let expire_duration = ((SystemTime::now() + Duration::from_secs(24 * 60 * 60))
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()) as usize;

    let claims = Claims {
        id: id.to_owned(),
        sub: email.to_owned(),
        exp: expire_duration,
    };

    let secret_key = CATAN_ENV.login_secret_key.clone();

    let token_result = encode(
        &Header::new(Algorithm::HS512),
        &claims,
        &EncodingKey::from_secret(secret_key.as_ref()),
    );

    token_result.map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
}
pub fn validate_jwt_token(token: &str) -> Option<TokenData<Claims>> {
    let validation = Validation::new(Algorithm::HS512);
    let secret_key = CATAN_ENV.login_secret_key.clone();

    match decode::<Claims>(
        &token,
        &DecodingKey::from_secret(secret_key.as_ref()),
        &validation,
    ) {
        Ok(c) => {
            full_info!("token VALID");
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
    data: &Data<ServiceEnvironmentContext>,
    is_test: bool,
) -> Result<ServiceResponse<Vec<ClientUser>>, ServiceResponse<String>> {
    let request_context = data.context.lock().unwrap();

    if !is_test {
        let userdb = UserDb::new(&request_context).await;

        // Get list of users
        match userdb.list().await {
            Ok(users) => {
                let client_users: Vec<ClientUser> = users
                    .iter()
                    .map(|user| ClientUser::from_persist_user(user.clone()))
                    .collect();

                Ok(ServiceResponse::new(
                    "",
                    StatusCode::OK,
                    client_users,
                    GameError::NoError,
                ))
            }
            Err(err) => {
                return Err(ServiceResponse::new(
                    "",
                    StatusCode::NOT_FOUND,
                    format!("Failed to retrieve user list: {}", err),
                    GameError::HttpError,
                ));
            }
        }
    } else {
        let client_users: Vec<ClientUser> = TestDb::list()
            .await
            .unwrap()
            .iter()
            .map(|user| ClientUser::from_persist_user(user.clone()))
            .collect();

        Ok(ServiceResponse::new(
            "",
            StatusCode::OK,
            client_users,
            GameError::NoError,
        ))
    }
}
pub async fn get_profile(
    user_id: &str,
    is_test: bool,
    data: &Data<ServiceEnvironmentContext>,
) -> Result<ServiceResponse<ClientUser>, ServiceResponse<String>> {
    let result = internal_find_user("id", user_id, is_test, &data).await;
    match result {
        Ok(user) => Ok(ServiceResponse::new(
            "",
            StatusCode::OK,
            ClientUser::from_persist_user(user),
            GameError::NoError,
        )),
        Err(err) => Err(ServiceResponse::new(
            "",
            StatusCode::NOT_FOUND,
            format!("FFailed to find user: {}", err),
            GameError::HttpError,
        )),
    }
}

pub async fn find_user_by_id(
    id: &str,
    is_test: bool,
    data: &Data<ServiceEnvironmentContext>,
) -> Result<ServiceResponse<ClientUser>, ServiceResponse<String>> {
    get_profile(id, is_test, data).await
}

pub async fn internal_find_user(
    prop: &str,
    id: &str,
    is_test: bool,
    data: &Data<ServiceEnvironmentContext>,
) -> Result<PersistUser, azure_core::Error> {
    if is_test {
        if prop == "id" {
            return TestDb::find_user_by_id(id).await;
        } else {
            return TestDb::find_user_by_profile(&prop, id).await;
        }
    }
    let request_context = data.context.lock().unwrap();
    let userdb = UserDb::new(&request_context).await;
    if prop == "id" {
        userdb.find_user_by_id(id).await
    } else {
        userdb.find_user_by_profile(&prop, id).await
    }
}

pub async fn delete(
    id_path: &str,
    id_token: &str,
    is_test: bool,
    data: &Data<ServiceEnvironmentContext>,
) -> Result<ServiceResponse<String>, ServiceResponse<String>> {
    //
    // unwrap is ok here because our middleware put it there.

    if id_path != id_token {
        return Err(ServiceResponse::new(
            "you can only delete yourself",
            StatusCode::UNAUTHORIZED,
            String::new(),
            GameError::HttpError,
        ));
    }

    let result;
    if !is_test {
        let request_context = data.context.lock().unwrap();
        let userdb = UserDb::new(&request_context).await;
        result = userdb.delete_user(&id_path).await
    } else {
        result = TestDb::delete_user(&id_path).await;
    }
    match result {
        Ok(..) => Ok(ServiceResponse::new(
            &format!("deleted user with id: {}", id_path),
            StatusCode::OK,
            String::new(),
            GameError::NoError,
        )),
        Err(err) => {
            return Err(ServiceResponse::new(
                "failed to delete user",
                StatusCode::BAD_REQUEST,
                String::new(),
                GameError::HttpError,
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test the login function
    #[tokio::test]
    async fn test_login() {
        let profile = UserProfile::new_test_user();
        let data = Data::new(ServiceEnvironmentContext::new()); // Customize with required fields

        // setup
        let response = setup(true, "db_name", "container_name").await;

        // Register the user first
        let sr = register(true, "password", &profile, &data)
            .await
            .expect("this should work");
        let client_user: ClientUser = sr.body;

        // Test login with correct credentials
        let response = login(&profile.email, "password", true, &data)
            .await
            .expect("login should succeed");

        // Test login with incorrect credentials
        let response = login(&profile.email, "wrong_password", true, &data).await;
        match response {
            Ok(_) => panic!("this hsould be an error!"),
            Err(e) => {
                assert_eq!(e.status, StatusCode::UNAUTHORIZED);
            }
        }

        // find user

        //  let user = find_user_by_id(id, is_test, &data)
    }

    // Similar tests for other functions: get_profile, find_user_by_id

    // Test JWT token creation and validation
    #[test]
    fn test_jwt_token_creation_and_validation() {
        let token = create_jwt_token("user_id", "user_email").unwrap();
        assert!(validate_jwt_token(&token).is_some());
    }

    // Add more tests as needed
}
