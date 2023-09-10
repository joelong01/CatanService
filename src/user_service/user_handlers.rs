use crate::{
    get_header_value,
    middleware::environment_mw::RequestContext,
    shared::{
        header_extractor::HeadersExtractor,
        models::{GameError, ResponseType, ServiceResponse, UserProfile, ClientUser},
    },
};
use actix_web::{
    web::{self},
    HttpResponse, Responder, http::Error,
};
use reqwest::StatusCode;

use super::users::{login, register, verify_cosmosdb};

/**
 * Handlers for the "user" service.
 * Each handler delegates to the implementations in ./users.rs.
 * Handles HttpRequest details using the HeaderExtractor from src/shared.
 * Handles all HttpResponse duties as well.
 */

// Set up the service
pub async fn verify_handler(request_context: RequestContext) -> HttpResponse {
    let result = verify_cosmosdb(&request_context).await;
    match result {
        Ok(sr) => sr.to_http_response(),
        Err(e) => e.to_http_response()
    }
}

// Register a new user
pub async fn register_handler(
    profile_in: web::Json<UserProfile>,
    request_context: RequestContext,
    headers: HeadersExtractor,
) -> impl Responder {
    let password = get_header_value!(password, headers);
    register(&password, &profile_in, &request_context)
        .await
        .map(|sr| sr.to_http_response())
        .unwrap_or_else(|sr| sr.to_http_response())
}

// User login
pub async fn login_handler(
    request_context: RequestContext,
    headers: HeadersExtractor,
) -> HttpResponse {
    let password = get_header_value!(password, headers);
    let username = get_header_value!(email, headers);
    login(&username, &password, &request_context)
        .await
        .map(|sr| sr.to_http_response())
        .unwrap_or_else(|sr| sr.to_http_response())
}

// List users
pub async fn list_users_handler(request_context: RequestContext) -> HttpResponse {
    super::users::list_users(&request_context)
        .await
        .map(|sr| sr.to_http_response())
        .unwrap_or_else(|sr| sr.to_http_response())
}

// Get user profile
pub async fn get_profile_handler(
    request_context: RequestContext,
    headers: HeadersExtractor,
) -> HttpResponse {
    let user_id = get_header_value!(user_id, headers);
    super::users::get_profile(&user_id, &request_context)
        .await
        .map(|sr| sr.to_http_response())
        .unwrap_or_else(|sr| sr.to_http_response())
}

// Find user by ID
pub async fn find_user_by_id_handler(
    id: web::Path<String>,
    request_context: RequestContext,
) -> Result<HttpResponse, Error> {
    match request_context.database.find_user_by_id(&id).await {
        Ok(option) => {
            match option {
                Some(user) => {
                    let service_response = ServiceResponse::new(
                        "",
                        StatusCode::OK,
                        ResponseType::ClientUser(ClientUser::from_persist_user(&user)),
                        GameError::NoError(String::default()),
                    );
                    Ok(service_response.to_http_response())
                }, 
                None => {
                    // Handle the case where the user is not found.
                    // You can modify this based on how you want to handle a missing user.
                    let service_response = ServiceResponse::new(
                        "",
                        StatusCode::NOT_FOUND,
                        ResponseType::NoData, // Assuming ClientUser has a default instance or you could use another placeholder
                        GameError::NoError(String::default()),
                    );
                    Ok(service_response.to_http_response())
                }
            }
        },
        Err(service_response) => {
            Ok(service_response.to_http_response())
        }
    }
}



// Delete user
pub async fn delete_handler(
    id: web::Path<String>,
    request_context: RequestContext,
    headers: HeadersExtractor,
) -> HttpResponse {
    let user_id = get_header_value!(user_id, headers);
    super::users::delete(&id, &user_id, &request_context)
        .await
        .map(|sr| sr.to_http_response())
        .unwrap_or_else(|sr| sr.to_http_response())
}

pub async fn validate_email(token: web::Path<String>) -> HttpResponse {
    super::users::validate_email(&token)
        .await
        .map(|sr| sr.to_http_response())
        .unwrap_or_else(|sr| sr.to_http_response())
}

pub fn create_http_response(status_code: StatusCode, message: &str, body: &str) -> HttpResponse {
    let response = ServiceResponse::new(
        message,
        status_code,
        ResponseType::Todo(body.to_string()),
        GameError::HttpError(status_code),
    );
    match status_code {
        StatusCode::OK => HttpResponse::Ok().json(response),
        StatusCode::UNAUTHORIZED => HttpResponse::Unauthorized().json(response),
        StatusCode::INTERNAL_SERVER_ERROR => HttpResponse::InternalServerError().json(response),
        StatusCode::NOT_FOUND => HttpResponse::NotFound().json(response),
        StatusCode::CONFLICT => HttpResponse::Conflict().json(response),
        StatusCode::ACCEPTED => HttpResponse::Accepted().json(response),
        StatusCode::CREATED => HttpResponse::Created().json(response),
        _ => HttpResponse::BadGateway().json(response),
    }
}

pub async fn validate_phone_handler(
    headers: HeadersExtractor,
    code: web::Path<String>,
    request_context: RequestContext,
) -> HttpResponse {
    let user_id = get_header_value!(user_id, headers);
    super::users::validate_phone(&user_id, &code, &request_context)
        .await
        .map(|sr| sr.to_http_response())
        .unwrap_or_else(|sr| sr.to_http_response())
}

pub async fn send_phone_code_handler(
    headers: HeadersExtractor,
    request_context: RequestContext,
) -> HttpResponse {
    let user_id = get_header_value!(user_id, headers);
    super::users::send_phone_code(&user_id, &request_context)
        .await
        .map(|sr| sr.to_http_response())
        .unwrap_or_else(|sr| sr.to_http_response())
}
