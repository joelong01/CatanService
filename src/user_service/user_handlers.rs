#![allow(dead_code)]
use crate::{
    api_call, get_header_value,
    middleware::{header_extractor::HeadersExtractor, request_context_mw::RequestContext},
    shared::shared_models::UserProfile,
};
use actix_web::{
    web::{self},
    HttpResponse, Responder,
};


/**
 * Handlers for the "user" service.
 * Each handler delegates to the implementations in ./users.rs.
 * Handles HttpRequest details using the HeaderExtractor from src/shared.
 * Handles all HttpResponse duties as well.
 */

// Set up the service
pub async fn verify_handler(request_context: RequestContext) -> HttpResponse {
    api_call!(super::users::verify_cosmosdb(&request_context).await)
}

// Register a new user
pub async fn register_handler(
    profile_in: web::Json<UserProfile>,
    request_context: RequestContext,
    headers: HeadersExtractor,
) -> impl Responder {
    let password = get_header_value!(password, headers);
    api_call!(super::users::register_user(&password, &profile_in, &request_context).await)
}

pub async fn register_test_user_handler(
    profile_in: web::Json<UserProfile>,
    request_context: RequestContext,
    headers: HeadersExtractor,
) -> impl Responder {
    let password = get_header_value!(password, headers);
    let storage_location = get_header_value!(profile_storage, headers);
    api_call!(super::users::register_test_user(storage_location, &password, &profile_in, &request_context).await)
}

// User login
pub async fn login_handler(
    request_context: RequestContext,
    headers: HeadersExtractor,
) -> HttpResponse {
    let login_data = get_header_value!(login_data, headers);
    api_call!(super::users::login(&login_data, &request_context).await)
}

// List users
pub async fn list_users_handler(request_context: RequestContext) -> HttpResponse {
    api_call!(super::users::list_users(&request_context).await)
}

// Get user profile
pub async fn get_profile_handler(
    email: web::Path<String>,
    request_context: RequestContext,
) -> HttpResponse {
    api_call!(super::users::get_profile(&email, &request_context).await)
}

pub async fn update_profile_handler(
    request_context: RequestContext,
    profile_in: web::Json<UserProfile>,
) -> HttpResponse {
    api_call!(super::users::update_profile(&profile_in, &request_context).await)
}

// Find user by ID
pub async fn find_user_by_id_handler(
    id: web::Path<String>,
    request_context: RequestContext,
) -> HttpResponse {
    api_call!(super::users::find_user_by_id(&id, &request_context).await)
}

// Delete user
pub async fn delete_handler(
    id: web::Path<String>,
    request_context: RequestContext,
) -> HttpResponse {
    api_call!(super::users::delete(&id, &request_context).await)
}

pub async fn validate_email_handler(token: web::Path<String>) -> HttpResponse {
    api_call!(super::users::validate_email(&token).await)
}

pub async fn validate_phone_handler(
    code: web::Path<String>,
    request_context: RequestContext,
) -> HttpResponse {
    api_call!(super::users::validate_phone(&code, &request_context).await)
}

pub async fn send_phone_code_handler(request_context: RequestContext) -> HttpResponse {
    api_call!(super::users::send_phone_code(&request_context).await)
}

pub async fn send_validation_email_handler(request_context: RequestContext) -> HttpResponse {
    api_call!(super::users::send_validation_email(&request_context))
}

pub async fn rotate_login_keys_handler(request_context: RequestContext) -> HttpResponse {
    api_call!(super::users::rotate_login_keys(&request_context).await)
}
pub async fn create_local_user_handler(
    profile_in: web::Json<UserProfile>,
    request_context: RequestContext,
) -> HttpResponse {
    api_call!(super::users::create_local_user(&profile_in, &request_context).await)
}

pub async fn update_local_user_handler(
    profile_in: web::Json<UserProfile>,
    request_context: RequestContext,
) -> HttpResponse {
    api_call!(super::users::update_local_user(&profile_in, &request_context).await)
}

pub async fn delete_local_user_handler(
    id: web::Path<String>,
    request_context: RequestContext,
) -> HttpResponse {
    api_call!(super::users::delete_local_user(&id, &request_context).await)
}
pub async fn get_local_users_handler(
    id: web::Path<String>,
    request_context: RequestContext,
) -> HttpResponse {
    api_call!(super::users::get_local_users(&id, &request_context).await)
}
