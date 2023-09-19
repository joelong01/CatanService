use crate::{
    get_header_value,
    middleware::{request_context_mw::RequestContext, header_extractor::HeadersExtractor},
    shared::shared_models::{UserProfile, GameError, ResponseType, ServiceResponse},
};
use actix_web::{
    http::Error,
    web::{self},
    HttpResponse, Responder,
};
use reqwest::StatusCode;

use super::users::{login, register, register_test_user, verify_cosmosdb};

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
        Err(e) => e.to_http_response(),
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

pub async fn register_test_user_handler(
    profile_in: web::Json<UserProfile>,
    request_context: RequestContext,
    headers: HeadersExtractor,
) -> impl Responder {
    let password = get_header_value!(password, headers);

    register_test_user(&password, &profile_in, &request_context)
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
pub async fn get_profile_handler(request_context: RequestContext) -> HttpResponse {
    super::users::get_profile(&request_context)
        .await
        .map(|sr| sr.to_http_response())
        .unwrap_or_else(|sr| sr.to_http_response())
}

pub async fn update_profile_handler(request_context: RequestContext,   profile_in: web::Json<UserProfile>) -> HttpResponse {
    super::users::update_profile(&profile_in, &request_context)
        .await
        .map(|sr| sr.to_http_response())
        .unwrap_or_else(|sr| sr.to_http_response())
}


// Find user by ID
pub async fn find_user_by_id_handler(
    request_context: RequestContext,
) -> Result<HttpResponse, Error> {
    let id = request_context
        .claims
        .expect("auth_mw should have added this or rejected the call")
        .id
        .clone();
    match request_context.database.find_user_by_id(&id).await {
        Ok(option) => {
            match option {
                Some(user) => {
                    let service_response = ServiceResponse::new(
                        "",
                        StatusCode::OK,
                        ResponseType::Profile(UserProfile::from_persist_user(&user)),
                        GameError::NoError(String::default()),
                    );
                    Ok(service_response.to_http_response())
                }
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
        }
        Err(service_response) => Ok(service_response.to_http_response()),
    }
}

// Delete user
pub async fn delete_handler(request_context: RequestContext) -> HttpResponse {
    super::users::delete(&request_context)
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
    code: web::Path<String>,
    request_context: RequestContext,
) -> HttpResponse {
    super::users::validate_phone(&code, &request_context)
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

pub async fn rotate_login_keys_handler(
    request_context: RequestContext,
) -> HttpResponse {

    super::users::rotate_login_keys(&request_context)
        .await
        .map(|sr| sr.to_http_response())
        .unwrap_or_else(|sr| sr.to_http_response())
}