use crate::{
    get_header_value,
    middleware::environment_mw::RequestContext,
    shared::{
        header_extractor::HeadersExtractor,
        models::{ServiceResponse, UserProfile, GameError, ResponseType},
    },
};
use actix_web::{
    web::{self},
    HttpResponse, Responder,
};
use reqwest::StatusCode;

use super::users::{login, register, setup};

/**
 * Handlers for the "user" service.
 * Each handler delegates to the implementations in ./users.rs.
 * Handles HttpRequest details using the HeaderExtractor from src/shared.
 * Handles all HttpResponse duties as well.
 */

// Set up the service
pub async fn setup_handler(
    request_context: RequestContext,
) -> HttpResponse {
   
    setup(&request_context)
    .await
    .map(|sr| sr.to_http_response())
    .unwrap_or_else(|sr| sr.to_http_response())
}

// Register a new user
pub async fn register_handler(
    profile_in: web::Json<UserProfile>,
    request_context: RequestContext,
    headers: HeadersExtractor,
) -> impl Responder {
    let password = get_header_value!(password, headers);
    register( &password, &profile_in, &request_context).await
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
    login(&username, &password, &request_context).await
    .map(|sr| sr.to_http_response())
    .unwrap_or_else(|sr| sr.to_http_response())
}

// List users
pub async fn list_users_handler(
    request_context: RequestContext,

) -> HttpResponse {
    super::users::list_users(&request_context).await
    .map(|sr| sr.to_http_response())
    .unwrap_or_else(|sr| sr.to_http_response())
}

// Get user profile
pub async fn get_profile_handler(
    request_context: RequestContext,
    headers: HeadersExtractor,
) -> HttpResponse {
    let user_id = get_header_value!(user_id, headers);
    super::users::get_profile(&user_id,  &request_context).await
    .map(|sr| sr.to_http_response())
    .unwrap_or_else(|sr| sr.to_http_response())
}

// Find user by ID
pub async fn find_user_by_id_handler(
    id: web::Path<String>,
    request_context: RequestContext,
) -> HttpResponse {
    super::users::find_user_by_id(&id, &request_context).await
    .map(|sr| sr.to_http_response())
    .unwrap_or_else(|sr| sr.to_http_response())
}

// Delete user
pub async fn delete_handler(
    id: web::Path<String>,
    request_context: RequestContext,
    headers: HeadersExtractor,
) -> HttpResponse {
    let user_id = get_header_value!(user_id, headers);
    super::users::delete(&id, &user_id, &request_context).await
    .map(|sr| sr.to_http_response())
    .unwrap_or_else(|sr| sr.to_http_response())
}

pub async fn validate_email(token: web::Path<String>, request_context: RequestContext) -> HttpResponse {
    super::users::validate_email(&token, &request_context).await
    .map(|sr| sr.to_http_response())
    .unwrap_or_else(|sr| sr.to_http_response())
}

pub fn create_http_response(status_code: StatusCode, message: &str, body: &str) -> HttpResponse {
    let response = ServiceResponse::new(
        message,
        status_code,
        ResponseType::Todo(body.to_string()),
        GameError::HttpError
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


