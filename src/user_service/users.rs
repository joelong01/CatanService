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
use crate::games_service::game_container::game_messages::GameHeader;
use crate::games_service::long_poller::long_poller::LongPoller;
use crate::middleware::environment_mw::{ServiceEnvironmentContext, CATAN_ENV};
use crate::shared::models::{Claims, ClientUser, PersistUser, ServiceResponse, UserProfile};
use actix_web::web::Data;
use actix_web::{web, HttpRequest, HttpResponse};
use azure_core::error::Result as AzureResult;
use azure_core::StatusCode;
use bcrypt::{hash, verify, DEFAULT_COST};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use lazy_static::lazy_static;
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
pub async fn setup(data: Data<ServiceEnvironmentContext>) -> HttpResponse {
    let request_context = data.context.lock().unwrap();
    //  do not check claims as the db doesn't exist yet.
    if !request_context.is_test {
        return create_http_response(
            StatusCode::Unauthorized,
            &format!("setup is only available when the test header is set."),
            "",
        );
    }
    if DB_SETUP.load(Ordering::Relaxed) {
        return create_http_response(
            StatusCode::Accepted,
            &format!(
                "database: {} container: {} already exists",
                request_context.database_name, request_context.env.container_name
            ),
            "",
        );
    }

    let _lock_guard = SETUP_LOCK.lock().await;

    if DB_SETUP.load(Ordering::Relaxed) {
        return create_http_response(
            StatusCode::Accepted,
            &format!(
                "database: {} container: {} already exists",
                request_context.database_name, request_context.env.container_name
            ),
            "",
        );
    }

    match TestDb::setupdb().await {
        Ok(..) => {
            DB_SETUP.store(true, Ordering::Relaxed);
            create_http_response(
                StatusCode::Created,
                &format!(
                    "database: {} container: {} created",
                    request_context.database_name, request_context.env.container_name
                ),
                "",
            )
        }
        Err(err) => create_http_response(
            StatusCode::BadRequest,
            &format!("Failed to create database/collection: {}", err),
            "",
        ),
    }
}

/// Registers a new user by hashing the provided password and creating a `PersistUser` record in the database.
///
/// # Arguments
///
/// * `user_in` - UserProfile object
/// * `data` - `ServiceEnvironmentContext` data.
/// * `req` - `HttpRequest` object containing the request information.
///
/// # Headers
/// * X-Password - The new password for the user
///
/// # Returns
/// Body contains a ClientUser (an id + profile)
/// Returns an HTTP response indicating the success or failure of the registration process.
pub async fn register(
    profile_in: web::Json<UserProfile>,
    data: Data<ServiceEnvironmentContext>,
    req: HttpRequest,
) -> HttpResponse {
    let is_test = req.headers().contains_key(GameHeader::IS_TEST);
    // Retrieve the password value from the "X-Password" header
    let password_value: String = match req.headers().get(GameHeader::PASSWORD) {
        Some(header_value) => match header_value.to_str() {
            Ok(pwd) => pwd.to_string(),
            Err(e) => {
                return create_http_response(
                    StatusCode::BadRequest,
                    &format!("X-Password header is set, but the value is not. Err: {}", e),
                    "",
                )
            }
        },
        None => {
            return create_http_response(
                StatusCode::BadRequest,
                &format!("X-Password header not set"),
                "",
            )
        }
    };
    let user_result = internal_find_user("email", &profile_in.email, is_test, &data).await;

    match user_result {
        Ok(_) => {
            return create_http_response(
                StatusCode::Conflict,
                &format!("User already registered: {}", profile_in.email),
                "",
            );
        }
        Err(_) => {
            // User not found, continue registration
        }
    }

    // Hash the password
    let password_hash = match hash(&password_value, DEFAULT_COST) {
        Ok(hp) => hp,
        Err(_) => {
            return create_http_response(
                StatusCode::InternalServerError,
                "Error hashing password",
                "",
            );
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
                HttpResponse::Ok()
                    .content_type("application/json")
                    .json(ClientUser::from_persist_user(persist_user))
            }
            Err(err) => create_http_response(
                StatusCode::BadRequest,
                &format!("Failed to add user to collection: {}", err),
                "",
            ),
        }
    } else {
        TestDb::create_user(persist_user.clone()).await.unwrap();
        HttpResponse::Ok()
            .content_type("application/json")
            .json(ClientUser::from_persist_user(persist_user))
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
pub async fn login(data: Data<ServiceEnvironmentContext>, req: HttpRequest) -> HttpResponse {
    let password_value: String = match req.headers().get(GameHeader::PASSWORD) {
        Some(header_value) => match header_value.to_str() {
            Ok(pwd) => pwd.to_string(),
            Err(e) => {
                return create_http_response(
                    StatusCode::BadRequest,
                    &format!(
                        "{} header is set, but the value is not. Err: {}",
                        GameHeader::PASSWORD,
                        e
                    ),
                    "",
                )
            }
        },
        None => {
            return create_http_response(
                StatusCode::BadRequest,
                &format!("{} header not set", GameHeader::PASSWORD),
                "",
            )
        }
    };

    let username = match req.headers().get(GameHeader::EMAIL) {
        Some(header_value) => match header_value.to_str() {
            Ok(name) => name,
            Err(e) => {
                return create_http_response(
                    StatusCode::BadRequest,
                    &format!(
                        "{} header is set, but the value is not. Err: {}",
                        GameHeader::EMAIL,
                        e
                    ),
                    "",
                )
            }
        },
        None => {
            return create_http_response(
                StatusCode::BadRequest,
                &format!("{} header not set", GameHeader::EMAIL),
                "",
            )
        }
    };
    let is_test = req.headers().contains_key(GameHeader::IS_TEST);
    let user_result = internal_find_user("email", username, is_test, &data).await;

    let user = match user_result {
        Ok(u) => u,
        Err(_) => {
            return create_http_response(
                StatusCode::NotFound,
                &format!("invalid user id: {}", username),
                "",
            );
        }
    };
    let password_hash = match user.password_hash {
        Some(p) => p,
        None => {
            return create_http_response(
                StatusCode::InternalServerError,
                "user document does not contain a password hash",
                "",
            );
        }
    };
    let is_password_match = verify(password_value, &password_hash).unwrap();

    if is_password_match {
        let expire_duration = ((SystemTime::now() + Duration::from_secs(24 * 60 * 60))
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()) as usize;
        let claims = Claims {
            id: user.id.clone(), // has to be there as we searched for it above
            sub: username.to_string(),
            exp: expire_duration,
        };

        let token_result = encode(
            &Header::new(Algorithm::HS512),
            &claims,
            &EncodingKey::from_secret(CATAN_ENV.login_secret_key.as_ref()),
        );

        match token_result {
            Ok(token) => {
                let _ = LongPoller::add_user(&user.id).await;
                create_http_response(StatusCode::Ok, "", &token)
            }
            Err(_) => {
                create_http_response(StatusCode::InternalServerError, "error hashing token", "")
            }
        }
    } else {
        create_http_response(StatusCode::Unauthorized, "incorrect password", "")
    }
}
/**
 *  this will get a list of all documents.  Note this does *not* do pagination. This would be a reasonable next step to
 *  show in the sample
 */
pub async fn list_users(data: Data<ServiceEnvironmentContext>, req: HttpRequest) -> HttpResponse {
    let request_context = data.context.lock().unwrap();
    let use_cosmos_db = !req.headers().contains_key(GameHeader::IS_TEST);
    if use_cosmos_db {
        let userdb = UserDb::new(&request_context).await;

        // Get list of users
        match userdb.list().await {
            Ok(users) => {
                let mut client_users = Vec::new();
                for user in users.iter() {
                    client_users.push(ClientUser::from_persist_user(user.clone()));
                }

                HttpResponse::Ok()
                    .content_type("application/json")
                    .json(client_users)
            }
            Err(err) => {
                return create_http_response(
                    StatusCode::NotFound,
                    &format!("Failed to retrieve user list: {}", err),
                    "",
                );
            }
        }
    } else {
        HttpResponse::Ok()
            .content_type("application/json")
            .json(TestDb::list().await.unwrap())
    }
}
pub async fn get_profile(data: Data<ServiceEnvironmentContext>, req: HttpRequest) -> HttpResponse {
    let user_id = req
        .headers()
        .get(GameHeader::USER_ID)
        .unwrap()
        .to_str()
        .unwrap();

    let is_test = req.headers().contains_key(GameHeader::IS_TEST);
    let result = internal_find_user("id", user_id, is_test, &data).await;
    match result {
        Ok(user) => HttpResponse::Ok()
            .content_type("application/json")
            .json(ClientUser::from_persist_user(user)),
        Err(err) => create_http_response(
            StatusCode::NotFound,
            &format!("Failed to find user: {}", err),
            "",
        ),
    }
}

/**
 *  this will get a list of all documents.  Note this does *not* do pagination. This would be a reasonable next step to
 *  show in the sample
 */
pub async fn find_user_by_id(
    id: web::Path<String>,
    data: Data<ServiceEnvironmentContext>,
    req: HttpRequest,
) -> HttpResponse {
    let is_test = req.headers().contains_key(GameHeader::IS_TEST);
    let result = internal_find_user("id", &id, is_test, &data).await;

    match result {
        Ok(mut user) => {
            user.password_hash = None;
            HttpResponse::Ok()
                .content_type("application/json")
                .json(user)
        }
        Err(err) => {
            create_http_response(
                StatusCode::NotFound,
                &format!("Failed to find user: {}", err),
                "",
            )
        }
    }
}

pub async fn internal_find_user(
    prop: &str,
    id: &str,
    is_test: bool,
    data: &Data<ServiceEnvironmentContext>,
) -> AzureResult<PersistUser> {
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
    id: web::Path<String>,
    data: Data<ServiceEnvironmentContext>,
    req: HttpRequest,
) -> HttpResponse {
    //
    // unwrap is ok here because our middleware put it there.
    let header_id_result = req.headers().get(GameHeader::USER_ID).unwrap().to_str();

    let header_id = match header_id_result {
        Ok(val) => val,
        Err(_) => {
            return create_http_response(StatusCode::BadRequest, "Invalid id header value", "")
        }
    };

    if header_id != *id {
        return create_http_response(StatusCode::Unauthorized, "you can only delete yourself", "");
    }
    let use_cosmos_db = !req.headers().contains_key(GameHeader::IS_TEST);
    let result;
    if use_cosmos_db {
        let request_context = data.context.lock().unwrap();
        let userdb = UserDb::new(&request_context).await;
        result = userdb.delete_user(&id).await
    } else {
        result = TestDb::delete_user(&id).await;
    }
    match result {
        Ok(..) => {
            create_http_response(
                StatusCode::Ok,
                &format!("deleted user with id: {}", id),
                "",
            )
        }
        Err(err) => create_http_response(
            StatusCode::BadRequest,
            &format!("Failed to delete user: {}", err),
            "",
        ),
    }
}

pub fn create_http_response(status_code: StatusCode, message: &str, body: &str) -> HttpResponse {
    let response = ServiceResponse {
        message: message.to_string(),
        status: status_code,
        body: body.to_string(),
    };
    match status_code {
        StatusCode::Ok => HttpResponse::Ok().json(response),
        StatusCode::Unauthorized => HttpResponse::Unauthorized().json(response),
        StatusCode::InternalServerError => HttpResponse::InternalServerError().json(response),
        StatusCode::NotFound => HttpResponse::NotFound().json(response),
        StatusCode::Conflict => HttpResponse::Conflict().json(response),
        StatusCode::Accepted => HttpResponse::Accepted().json(response),
        StatusCode::Created => HttpResponse::Created().json(response),
        _ => HttpResponse::BadRequest().json(response),
    }
}
