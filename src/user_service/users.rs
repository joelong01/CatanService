use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/**
 * this module implements the WebApi to create the database/collection, list all the users, and to create/find/delete
 * a User document in CosmosDb
 */
use crate::cosmos_db::cosmosdb::UserDb;
use crate::games_service::game_container::game_messages::GameHeaders;
use crate::games_service::lobby::lobby::Lobby;
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
            format!("setup is only available when the test header is set."),
            "".to_owned(),
        );
    }
    if DB_SETUP.load(Ordering::Relaxed) {
        return create_http_response(
            StatusCode::Accepted,
            format!(
                "database: {} container: {} already exists",
                request_context.database_name, request_context.env.container_name
            ),
            "".to_owned(),
        );
    }

    let _lock_guard = SETUP_LOCK.lock().await;

    if DB_SETUP.load(Ordering::Relaxed) {
        return create_http_response(
            StatusCode::Accepted,
            format!(
                "database: {} container: {} already exists",
                request_context.database_name, request_context.env.container_name
            ),
            "".to_owned(),
        );
    }

    let userdb = UserDb::new(&request_context).await;
    match userdb.setupdb().await {
        Ok(..) => {
            DB_SETUP.store(true, Ordering::Relaxed);
            create_http_response(
                StatusCode::Created,
                format!(
                    "database: {} container: {} created",
                    request_context.database_name, request_context.env.container_name
                ),
                "".to_owned(),
            )
        }
        Err(err) => create_http_response(
            StatusCode::BadRequest,
            format!("Failed to create database/collection: {}", err),
            "".to_owned(),
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
    // Retrieve the password value from the "X-Password" header
    let password_value: String = match req.headers().get(GameHeaders::PASSWORD) {
        Some(header_value) => match header_value.to_str() {
            Ok(pwd) => pwd.to_string(),
            Err(e) => {
                return create_http_response(
                    StatusCode::BadRequest,
                    format!("X-Password header is set, but the value is not. Err: {}", e),
                    "".to_owned(),
                )
            }
        },
        None => {
            return create_http_response(
                StatusCode::BadRequest,
                format!("X-Password header not set"),
                "".to_owned(),
            )
        }
    };

    // Check if the user already exists
    let user_result = internal_find_user("email", &profile_in.email, &data).await;

    match user_result {
        Ok(_) => {
            return create_http_response(
                StatusCode::Conflict,
                format!("User already registered: {}", profile_in.email),
                "".to_owned(),
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
                "Error hashing password".to_owned(),
                "".to_owned(),
            );
        }
    };

    // Create the user record
    let mut persist_user = PersistUser::from_user_profile(&profile_in, password_hash.to_owned());
    // ignore the game stats passed in by the client and create a new one
    persist_user.user_profile.games_played = Some(0);
    persist_user.user_profile.games_won = Some(0);
    // Create the database connection
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
            format!("Failed to add user to collection: {}", err),
            "".to_owned(),
        ),
    }
}

/**
 * login to the system.
 * a cleartext password is passed in (depending on HTTPS to stop MitM attacks and encrypt payload)
 * find the user in the database
 * hash the password and make sure it matches the hash in the db
 * if it does, return a signed JWT token
 */
pub async fn login(data: Data<ServiceEnvironmentContext>, req: HttpRequest) -> HttpResponse {
    let password_value: String = match req.headers().get(GameHeaders::PASSWORD) {
        Some(header_value) => match header_value.to_str() {
            Ok(pwd) => pwd.to_string(),
            Err(e) => {
                return create_http_response(
                    StatusCode::BadRequest,
                    format!(
                        "{} header is set, but the value is not. Err: {}",
                        GameHeaders::PASSWORD,
                        e
                    ),
                    "".to_owned(),
                )
            }
        },
        None => {
            return create_http_response(
                StatusCode::BadRequest,
                format!("{} header not set", GameHeaders::PASSWORD),
                "".to_owned(),
            )
        }
    };

    let username = match req.headers().get(GameHeaders::EMAIL) {
        Some(header_value) => match header_value.to_str() {
            Ok(name) => name,
            Err(e) => {
                return create_http_response(
                    StatusCode::BadRequest,
                    format!(
                        "{} header is set, but the value is not. Err: {}",
                        GameHeaders::EMAIL,
                        e
                    ),
                    "".to_owned(),
                )
            }
        },
        None => {
            return create_http_response(
                StatusCode::BadRequest,
                format!("{} header not set", GameHeaders::EMAIL),
                "".to_owned(),
            )
        }
    };

    let user_result = internal_find_user("email", username, &data).await;

    let user = match user_result {
        Ok(u) => u,
        Err(_) => {
            return create_http_response(
                StatusCode::NotFound,
                format!("invalid user id: {}", username),
                "".to_owned(),
            );
        }
    };
    let password_hash = match user.password_hash {
        Some(p) => p,
        None => {
            return create_http_response(
                StatusCode::InternalServerError,
                "user document does not contain a password hash".to_owned(),
                "".to_owned(),
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
                Lobby::join_lobby(user.id).await;
                create_http_response(StatusCode::Ok, "".to_owned(), token)
            }
            Err(_) => create_http_response(
                StatusCode::InternalServerError,
                "error hashing token".to_owned(),
                "".to_owned(),
            ),
        }
    } else {
        create_http_response(
            StatusCode::Unauthorized,
            "incorrect password".to_owned(),
            "".to_owned(),
        )
    }
}
/**
 *  this will get a list of all documents.  Note this does *not* do pagination. This would be a reasonable next step to
 *  show in the sample
 */
pub async fn list_users(data: Data<ServiceEnvironmentContext>) -> HttpResponse {
    let request_context = data.context.lock().unwrap();
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
            let response = ServiceResponse {
                message: format!("Failed to retrieve user list: {}", err),
                status: StatusCode::NotFound,
                body: "".to_owned(),
            };
            HttpResponse::NotFound()
                .content_type("application/json")
                .json(response)
        }
    }
}
pub async fn get_profile(data: Data<ServiceEnvironmentContext>, req: HttpRequest) -> HttpResponse {
    let header_id = req
        .headers()
        .get(GameHeaders::USER_ID)
        .unwrap()
        .to_str()
        .unwrap();
    match internal_find_user("id", header_id, &data).await {
        Ok(user) => HttpResponse::Ok()
            .content_type("application/json")
            .json(ClientUser::from_persist_user(user)),
        Err(err) => {
            let response = ServiceResponse {
                message: format!("Failed to find user: {}", err),
                status: StatusCode::NotFound,
                body: "".to_owned(),
            };
            HttpResponse::NotFound()
                .content_type("application/json")
                .json(response)
        }
    }
}

/**
 *  this will get a list of all documents.  Note this does *not* do pagination. This would be a reasonable next step to
 *  show in the sample
 */
pub async fn find_user_by_id(
    id: web::Path<String>,
    data: Data<ServiceEnvironmentContext>,
) -> HttpResponse {
    match internal_find_user("id", &id, &data).await {
        Ok(mut user) => {
            user.password_hash = None;
            HttpResponse::Ok()
                .content_type("application/json")
                .json(user)
        }
        Err(err) => {
            let response = ServiceResponse {
                message: format!("Failed to find user: {}", err),
                status: StatusCode::NotFound,
                body: "".to_owned(),
            };
            HttpResponse::NotFound()
                .content_type("application/json")
                .json(response)
        }
    }
}

pub async fn internal_find_user(
    prop: &str,
    id: &str,
    data: &Data<ServiceEnvironmentContext>,
) -> AzureResult<PersistUser> {
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
    let header_id_result = req.headers().get(GameHeaders::USER_ID).unwrap().to_str();

    let header_id = match header_id_result {
        Ok(val) => val,
        Err(_) => {
            return create_http_response(
                StatusCode::BadRequest,
                "Invalid id header value".to_owned(),
                "".to_owned(),
            )
        }
    };

    if header_id != *id {
        return create_http_response(
            StatusCode::Unauthorized,
            "you can only delete yourself".to_owned(),
            "".to_owned(),
        );
    }

    let request_context = data.context.lock().unwrap();
    let userdb = UserDb::new(&request_context).await;
    match userdb.delete_user(&id).await {
        Ok(..) => {
            let response = ServiceResponse {
                message: format!("deleted user with id: {}", id),
                status: StatusCode::Ok,
                body: "".to_owned(),
            };
            HttpResponse::Ok()
                .content_type("application/json")
                .json(response)
        }
        Err(err) => create_http_response(
            StatusCode::BadRequest,
            format!("Failed to delete user: {}", err),
            "".to_owned(),
        ),
    }
}

pub fn create_http_response(
    status_code: StatusCode,
    message: String,
    body: String,
) -> HttpResponse {
    let response = ServiceResponse {
        message: message,
        status: status_code,
        body: body,
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
