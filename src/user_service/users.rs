use std::time::{Duration, SystemTime, UNIX_EPOCH};

/**
 * this module implements the WebApi to create the database/collection, list all the users, and to create/find/delete
 * a User document in CosmosDb
 */
use crate::cosmos_db::cosmosdb::UserDb;
use crate::middleware::environment_mw::{ServiceEnvironmentContext, CATAN_ENV};
use crate::shared::models::{Claims, ClientUser, Credentials, PersistUser, ServiceResponse};
use crate::shared::utility::get_id;
use actix_web::web::Data;
use actix_web::{web, HttpRequest, HttpResponse};
use azure_core::error::Result as AzureResult;
use azure_core::StatusCode;
use bcrypt::{hash, verify, DEFAULT_COST};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};

/**
 * this sets up CosmosDb to make the sample run. the only prereq is the secrets set in
 * .devconainter/required-secrets.json, this API will call setupdb. this just calls the setupdb api and deals with errors
 *
 * you can't do the normal authn/authz here because the authn path requires the database to exist.  for this app,
 * the users database will be created out of band and this path is just for test users.
 */
pub async fn setup(data: Data<ServiceEnvironmentContext>) -> HttpResponse {
    //  do not check claims as the db doesn't exist yet.

    let request_context = data.context.lock().unwrap();
    if !request_context.is_test {
        return create_http_response(
            StatusCode::Unauthorized,
            format!("setup is only available when the test header is set."),
            "".to_owned(),
        );
    }
    let userdb = UserDb::new(&request_context).await;
    match userdb.setupdb().await {
        Ok(..) => {
            let response = ServiceResponse {
                message: format!(
                    "database: {} container: {} created",
                    request_context.database_name, request_context.env.container_name
                ),
                status: StatusCode::Ok,
                body: "".to_owned(),
            };
            HttpResponse::Ok()
                .content_type("application/json")
                .json(response)
        }
        Err(err) => {
            let response = ServiceResponse {
                message: format!("Failed to create database/collection: {}", err),
                status: StatusCode::BadRequest,
                body: "".to_owned(),
            };
            HttpResponse::BadRequest()
                .content_type("application/json")
                .json(response)
        }
    }
}

/// Registers a new user by hashing the provided password and creating a `PersistUser` record in the database.
///
/// # Arguments
///
/// * `user_in` - ClientUser object
/// * `data` - `ServiceEnvironmentContext` data.
/// * `req` - `HttpRequest` object containing the request information.
///
/// # Returns
///
/// Returns an HTTP response indicating the success or failure of the registration process.
pub async fn register(
    user_in: web::Json<ClientUser>,
    data: Data<ServiceEnvironmentContext>,
    req: HttpRequest,
) -> HttpResponse {
    // Retrieve the password value from the "X-Password" header
    let password_value: String = match req.headers().get("X-Password") {
        Some(header_value) => match header_value.to_str() {
            Ok(pwd) => pwd.to_string(),
            Err(e) => {
                let response = ServiceResponse {
                    message: format!("X-Password header is set, but the value is not. Err: {}", e),
                    status: StatusCode::BadRequest,
                    body: "".to_owned(),
                };
                return HttpResponse::BadRequest()
                    .content_type("application/json")
                    .json(response);
            }
        },
        None => {
            let response = ServiceResponse {
                message: format!("X-Password header not set"),
                status: StatusCode::BadRequest,
                body: "".to_owned(),
            };
            return HttpResponse::BadRequest()
                .content_type("application/json")
                .json(response);
        }
    };

    // Check if the user already exists
    let user_result = internal_find_user(
        "email".to_string(),
        user_in.user_profile.email.clone(),
        data.clone(),
    )
    .await;

    match user_result {
        Ok(_) => {
            return create_http_response(
                StatusCode::Conflict,
                format!("User already registered: {}", user_in.user_profile.email),
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
                StatusCode::Conflict,
                "Error hashing password".to_owned(),
                "".to_owned(),
            );
        }
    };

    // Create the user record
    let mut persist_user = PersistUser::from_client_user(&user_in, password_hash.to_owned());
    // ignore the id passed in by the client and create a new one
    persist_user.id = get_id();
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
        Err(err) => {
            let response = ServiceResponse {
                message: format!("Failed to add user to collection: {}", err),
                status: StatusCode::BadRequest,
                body: "".to_owned(),
            };
            HttpResponse::BadRequest()
                .content_type("application/json")
                .json(response)
        }
    }
}

/**
 * login to the system.
 * a cleartext password is passed in (depending on HTTPS to stop MitM attacks and encrypt payload)
 * find the user in the database
 * hash the password and make sure it matches the hash in the db
 * if it does, return a signed JWT token
 */
pub async fn login(
    creds: web::Json<Credentials>,
    data: Data<ServiceEnvironmentContext>,
) -> HttpResponse {
    let user_result = internal_find_user("email".to_string(), creds.username.clone(), data).await;

    let user = match user_result {
        Ok(u) => u,
        Err(_) => {
            return create_http_response(
                StatusCode::NotFound,
                format!("invalid user id: {}", creds.username),
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
    let is_password_match = verify(&creds.password, &password_hash).unwrap();

    if is_password_match {
        let expire_duration = ((SystemTime::now() + Duration::from_secs(24 * 60 * 60))
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()) as usize;
        let claims = Claims {
            id: user.id, // has to be there as we searched for it above
            sub: creds.username.clone(),
            exp: expire_duration,
        };

        let token_result = encode(
            &Header::new(Algorithm::HS512),
            &claims,
            &EncodingKey::from_secret(CATAN_ENV.login_secret_key.as_ref()),
        );

        match token_result {
            Ok(token) => create_http_response(StatusCode::Ok, "".to_owned(), token),
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
        Ok(mut users) => {
            for user in users.iter_mut() {
                user.password_hash = None;
            }

            HttpResponse::Ok()
                .content_type("application/json")
                .json(users)
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
   
    let header_id = req.headers().get("user_id").unwrap().to_str().unwrap();
    match internal_find_user("id".to_string(), header_id.to_string(), data).await {
        Ok(user) => {
            HttpResponse::Ok()
                .content_type("application/json")
                .json(user.user_profile)
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


/**
 *  this will get a list of all documents.  Note this does *not* do pagination. This would be a reasonable next step to
 *  show in the sample
 */
pub async fn find_user_by_id(
    id: web::Path<String>,
    data: Data<ServiceEnvironmentContext>,
) -> HttpResponse {
    match internal_find_user("id".to_string(), id.to_string(), data).await {
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
    prop: String,
    id: String,
    data: Data<ServiceEnvironmentContext>,
) -> AzureResult<PersistUser> {
    let request_context = data.context.lock().unwrap();
    let userdb = UserDb::new(&request_context).await;
    if prop == "id" {
        userdb.find_user_by_id(&id).await
    } else {
        userdb.find_user_by_profile(&prop, &id).await
    }

   
}

pub async fn delete(
    id: web::Path<String>,
    data: Data<ServiceEnvironmentContext>,
    req: HttpRequest,
) -> HttpResponse {
    //
    // unwrap is ok here because our middleware put it there.
    let header_id_result = req.headers().get("user_id").unwrap().to_str();

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
        Err(err) => {
            let response = ServiceResponse {
                message: format!("Failed to delete user: {}", err),
                status: StatusCode::BadRequest,
                body: "".to_owned(),
            };
            HttpResponse::BadRequest()
                .content_type("application/json")
                .json(response)
        }
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
        _ => HttpResponse::BadRequest().json(response),
    }
}
