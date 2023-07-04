use std::time::{Duration, SystemTime, UNIX_EPOCH};

/**
 * this module implements the WebApi to create the database/collection, list all the users, and to create/find/delete
 * a User document in CosmosDb
 */
use crate::cosmos_db::cosmosdb::UserDb;
use crate::shared::models::{CatanSecrets, Claims, Credentials, ServiceResponse, User};
use crate::shared::utility::{get_id, COLLECTION_NAME, DATABASE_NAME};
use actix_web::web::Data;
use actix_web::{web, HttpRequest, HttpResponse};
use azure_core::error::Result as AzureResult;
use azure_core::StatusCode;
use bcrypt::{hash, verify, DEFAULT_COST};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};

use super::middleware::AppState;
/**
 * this sets up CosmosDb to make the sample run. the only prereq is the secrets set in
 * .devconainter/required-secrets.json, this API will call setupdb. this just calls the setupdb api and deals with errors
 */
pub async fn setup(data: Data<AppState>) -> HttpResponse {
    //
    //  TODO:  if this is not a test, then only an admin can run setup, as it is destructive

    let request_info = data.request_info.lock().unwrap();
    let userdb = UserDb::new(&request_info.database, &request_info.collection).await;
    match userdb.setupdb().await {
        Ok(..) => {
            let response = ServiceResponse {
                message: format!(
                    "database: {} collection: {} \ncreated",
                    DATABASE_NAME, COLLECTION_NAME
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

/**
 *  this creates a user.  it uses web forms to collect the data from the client.  Note that if you are using PostMan
 *  to call this API, set the form data in 'x-www-form-urlencoded', *not* in 'form-data', as that will fail with a
 *  hard-to-figure-out error in actix_web deserialize layer.
 */
pub async fn register(user_in: web::Json<User>, data: Data<AppState>) -> HttpResponse {
    //TODO:  look up user by email to ensure that they aren't already in the system

    let mut user = user_in.clone();
    user.partition_key = Some(1);

    let password = match user.password {
        Some(p) => p,
        None => {
            let response = ServiceResponse {
                message: format!("missing password"),
                status: StatusCode::BadRequest,
                body: "".to_owned(),
            };
            return HttpResponse::BadRequest()
                .content_type("application/json")
                .json(response);
        }
    };

    user.password_hash = match hash(&password, DEFAULT_COST) {
        Ok(hp) => Some(hp),
        Err(_) => {
            return HttpResponse::InternalServerError().body("Error hashing password");
        }
    };
    user.password = None;
    user.id = Some(get_id());
    let request_info = data.request_info.lock().unwrap();
    let userdb = UserDb::new(&request_info.database, &request_info.collection).await;

    match userdb.create_user(user.clone()).await {
        Ok(..) => HttpResponse::Ok()
            .content_type("application/json")
            .json(user),
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
 *  this will get a list of all documents.  Note this does *not* do pagination. This would be a reasonable next step to
 *  show in the sample
 */
pub async fn list_users(data: Data<AppState>, req: HttpRequest) -> HttpResponse {
    let claims: Claims = match check_jwt(req) {
        Ok(c) => c,
        Err(e) => return e,
    };

    log::debug!("user {} is listing all users", claims.sub);

    let request_info = data.request_info.lock().unwrap();
    let userdb = UserDb::new(&request_info.database, &request_info.collection).await;

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
/**
 *  this will get a list of all documents.  Note this does *not* do pagination. This would be a reasonable next step to
 *  show in the sample
 */
pub async fn find_user_by_id(
    id: web::Path<String>,
    data: Data<AppState>,
    req: HttpRequest,
) -> HttpResponse {
    let _ = match check_jwt(req) {
        Ok(c) => c,
        Err(e) => return e,
    };
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

async fn internal_find_user(prop: String, id: String, data: Data<AppState>) -> AzureResult<User> {
    let request_info = data.request_info.lock().unwrap();
    let userdb = UserDb::new(&request_info.database, &request_info.collection).await;

    // Get list of users
    userdb.find_user(&prop, &id).await
}

pub async fn delete(id: web::Path<String>, data: Data<AppState>, req: HttpRequest) -> HttpResponse {
    let claim = match check_jwt(req) {
        Ok(c) => c,
        Err(e) => return e,
    };

    if claim.id != *id {
        return create_http_response(
            StatusCode::Unauthorized,
            "you can only delete yourself".to_owned(),
            "".to_owned(),
        );
    }

    let request_info = data.request_info.lock().unwrap();
    let userdb = UserDb::new(&request_info.database, &request_info.collection).await;
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

/**
 * login to the system.
 * a cleartext password is passed in (depending on HTTPS to stop MitM attacks and encrypt payload)
 * find the user in the database
 * hash the password and make sure it matches the hash in the db
 * if it does, return a signed JWT token
 */
pub async fn login(creds: web::Json<Credentials>, data: Data<AppState>) -> HttpResponse {
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
            id: user.id.unwrap(), // has to be there as we searched for it above
            sub: creds.username.clone(),
            exp: expire_duration,
        };

        let secrets = CatanSecrets::load_from_env().expect("Error loading secrets");
        let token_result = encode(
            &Header::new(Algorithm::HS512),
            &claims,
            &EncodingKey::from_secret(secrets.login_secret_key.as_ref()),
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
fn create_http_response(status_code: StatusCode, message: String, body: String) -> HttpResponse {
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
        _ => HttpResponse::BadRequest().json(response),
    }
}

fn check_jwt(req: HttpRequest) -> Result<Claims, HttpResponse> {
    let secrets = CatanSecrets::load_from_env().expect("Error loading secrets");
    let headers = req.headers();
    match headers.get("Authorization") {
        Some(value) => {
            let token_str = value.to_str().unwrap_or_default().replace("Bearer ", "");
            let validation = Validation::new(Algorithm::HS512);
            match decode::<Claims>(
                &token_str,
                &DecodingKey::from_secret(secrets.login_secret_key.as_ref()),
                &validation,
            ) {
                Ok(token_data) => {
                    // Token is valid. You can use the data in the token to perform authorization checks now.
                    Ok(token_data.claims)
                }
                Err(err) => {
                    Err(HttpResponse::Unauthorized().body(format!("Unauthorized: {}", err)))
                }
            }
        }
        None => Err(HttpResponse::Unauthorized().body("No Authorization Header")),
    }
}
