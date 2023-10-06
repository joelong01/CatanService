#![allow(dead_code)]

use actix_web::http;
use actix_web::http::header::{self, HeaderName, HeaderValue};
use serde::de::DeserializeOwned;

use crate::full_info;
use crate::games_service::catan_games::games::regular::regular_game::RegularGame;
use crate::games_service::game_container::game_messages::{
    CatanMessage, GameHeader, Invitation, InvitationResponseData,
};
use crate::games_service::shared::game_enums::{CatanGameType, GameAction};
use crate::middleware::request_context_mw::TestCallContext;
use crate::shared::service_models::LoginHeaderData;
use crate::shared::shared_models::UserProfile;
use crate::shared::shared_models::{ProfileStorage, ServiceError};
use actix_web::test::{self, TestRequest};
use serde::Serialize;

use actix_http::{Method, Request};
use actix_service::Service;
use actix_web::dev::ServiceResponse as ActixServiceResponse;
use actix_web::web::Bytes;
use actix_web::{
    body::{BoxBody, EitherBody},
    Error,
};

use std::collections::HashMap;

pub struct TestProxy<'a, S> {
    service: &'a S,
    auth_token: Option<String>,
}

impl<'a, S> TestProxy<'a, S>
where
    S: Service<Request, Response = ActixServiceResponse<EitherBody<BoxBody>>, Error = Error>,
    S::Future: 'static,
{
    pub fn new(service: &'a S) -> Self {
        TestProxy {
            service,
            auth_token: None,
        }
    }

    pub fn set_auth_token(&mut self, auth_token: Option<String>) {
        self.auth_token = auth_token.clone();
    }

    pub async fn send_request<B, T>(
        &self,
        method: Method,
        url: &str,
        headers: Option<&HashMap<HeaderName, HeaderValue>>,
        body: Option<B>,
    ) -> Result<T, ServiceError>
    where
        B: Serialize,
        T: DeserializeOwned + 'static,
    {
        full_info!("calling url: {}:{}", method, url);

        let mut request = match method {
            Method::GET => TestRequest::get().uri(url),
            Method::POST => TestRequest::post().uri(url),
            Method::PUT => TestRequest::put().uri(url),
            Method::DELETE => TestRequest::delete().uri(url),
            _ => {
                return Err(ServiceError::new_internal_server_fault(
                    "Unsupported HTTP method",
                ))
            }
        };

        // Always process headers if they're present
        if let Some(header_map) = headers {
            for (key, value) in header_map {
                request = request.append_header((key, value));
            }
        }

        // Only process bodies for PUT and POST methods
        if method == Method::PUT || method == Method::POST {
            // Adding Content-Type header for JSON
            request = request.append_header((header::CONTENT_TYPE, "application/json"));

            if let Some(body_content) = body {
                match serde_json::to_vec(&body_content) {
                    Ok(bytes) => {
                        request = request.set_payload(Bytes::from(bytes));
                    }
                    Err(e) => {
                        return Err(ServiceError::new_json_error(
                            "setting payload in TestProxy::send_request",
                            &e,
                        )); // Assuming you have such a method or variant
                    }
                }
            }
        }

        //
        // auth header
        if let Some(auth_token) = &self.auth_token {
            let header_value = format!("Bearer {}", auth_token);
            request = request.append_header(("Authorization", header_value));
        }

        let request = request.to_request();

        let response = test::call_service(self.service, request).await;
        full_info!("api returned status: {}", response.status());
        match response.status() {
            http::StatusCode::OK => {
                let parsed_body: T = test::try_read_body_json(response)
                    .await
                    .expect("Failed to parse the response body");
                Ok(parsed_body)
            }
            _ => {
                let error_response: ServiceError = test::try_read_body_json(response)
                    .await
                    .expect("Failed to parse the error response");
                Err(error_response)
            }
        }
    }

    pub async fn post<B, T>(
        &self,
        url: &str,
        headers: Option<&HashMap<HeaderName, HeaderValue>>,
        body: Option<B>,
    ) -> Result<T, ServiceError>
    where
        B: Serialize,
        T: DeserializeOwned + 'static,
    {
        self.send_request(Method::POST, url, headers, body).await
    }
    pub async fn get<T>(
        &self,
        url: &str,
        headers: Option<&HashMap<HeaderName, HeaderValue>>,
    ) -> Result<T, ServiceError>
    where
        T: DeserializeOwned + 'static,
    {
        self.send_request::<(), T>(Method::GET, url, headers, None)
            .await
    }

    pub async fn put<B, T>(
        &self,
        url: &str,
        headers: Option<&HashMap<HeaderName, HeaderValue>>,
        body: Option<B>,
    ) -> Result<T, ServiceError>
    where
        B: Serialize,
        T: DeserializeOwned + 'static,
    {
        self.send_request(Method::PUT, url, headers, body).await
    }

    pub async fn delete<T>(
        &self,
        url: &str,
        headers: Option<&HashMap<HeaderName, HeaderValue>>,
    ) -> Result<T, ServiceError>
    where
        T: DeserializeOwned + 'static,
    {
        self.send_request::<(), T>(Method::DELETE, url, headers, None)
            .await
    }
    pub async fn register(
        &self,
        profile: &UserProfile,
        password: &str,
    ) -> Result<UserProfile, ServiceError> {
        let mut headers: HashMap<HeaderName, HeaderValue> = HashMap::new();

        headers.insert(
            HeaderName::from_static(GameHeader::PASSWORD),
            HeaderValue::from_str(password).expect("Invalid header value"),
        );

        let url = "/api/v1/users/register";
        self.post::<&UserProfile, UserProfile>(url, Some(&headers), Some(profile))
            .await
    }

    pub async fn register_test_user(
        &self,
        location: ProfileStorage,
        profile: &UserProfile,
        password: &str,
    ) -> Result<UserProfile, ServiceError> {
        let mut headers: HashMap<HeaderName, HeaderValue> = HashMap::new();

        headers.insert(
            HeaderName::from_static(GameHeader::PASSWORD),
            HeaderValue::from_str(password).expect("Invalid header value"),
        );

        headers.insert(
            HeaderName::from_static(GameHeader::PROFILE_LOCATION),
            HeaderValue::from_str(
                &serde_json::to_string(&location)
                    .expect("serde serialization of an enum to not fail"),
            )
            .expect("Invalid header value"),
        );

        let url = "/auth/api/v1/users/register-test-user";
        self.post::<&UserProfile, UserProfile>(url, Some(&headers), Some(profile))
            .await
    }

    pub async fn login(&self, login_data: &LoginHeaderData) -> Result<String, ServiceError> {
        let mut headers: HashMap<HeaderName, HeaderValue> = HashMap::new();
        let json = serde_json::to_string(login_data).expect("serde serialization should work");
        headers.insert(
            HeaderName::from_static(GameHeader::LOGIN_DATA),
            HeaderValue::from_str(&json).expect("Invalid header value"),
        );

        let url = "/api/v1/users/login";
        self.post::<(), String>(url, Some(&headers), None).await
    }

    pub async fn test_login(
        &self,
        user_name: &str,
        password: &str,
    ) -> Result<String, ServiceError> {
        self.login(&LoginHeaderData::new(
            user_name,
            password,
            ProfileStorage::CosmosDbTest,
        ))
        .await
    }

    pub async fn setup(&self) -> Result<(), ServiceError> {
        let url = "/api/v1/test/verify-service";
        let sr = self.post::<(), ()>(url, None, None).await;
        sr
    }
    pub async fn get_profile(&self, id: &str) -> Result<UserProfile, ServiceError> {
        let url = format!("/auth/api/v1/profile/{}", id);

        self.get(&url, None).await
    }

    pub async fn new_game(
        &self,
        game_type: CatanGameType,
        game: Option<&RegularGame>,
    ) -> Result<RegularGame, ServiceError> {
        let url = format!("/auth/api/v1/games/{:?}", game_type);
        let mut headers: HashMap<HeaderName, HeaderValue> = HashMap::new();
        let test_call_context = TestCallContext::new(None, game.cloned());
        let json = serde_json::to_string(&test_call_context).unwrap();
        headers.insert(
            HeaderName::from_static(GameHeader::TEST),
            HeaderValue::from_str(&json).expect("Invalid header value"),
        );

        let service_error = match game {
            Some(g) => self.post(&url, None, Some(g)).await,
            None => self.post::<(), RegularGame>(&url, None, None).await,
        };

        service_error
    }

    pub async fn get_lobby(&self) -> Result<Vec<UserProfile>, ServiceError> {
        let url = "/auth/api/v1/lobby";
        self.get::<Vec<UserProfile>>(url, None).await
    }

    pub async fn get_actions(&self, game_id: &str) -> Result<Vec<GameAction>, ServiceError> {
        let url = format!("/auth/api/v1/action/actions/{}", game_id);
        let mut headers: HashMap<HeaderName, HeaderValue> = HashMap::new();

        headers.insert(
            HeaderName::from_static(GameHeader::GAME_ID),
            HeaderValue::from_str(game_id).expect("Invalid header value"),
        );

        self.get::<Vec<GameAction>>(&url, Some(&headers)).await
    }

    pub async fn long_poll(&self, game_id: &str, index: u32) -> Result<CatanMessage, ServiceError> {
        let url = format!("/auth/api/v1/longpoll/{}", index);
        let mut headers: HashMap<HeaderName, HeaderValue> = HashMap::new();

        headers.insert(
            HeaderName::from_static(GameHeader::GAME_ID),
            HeaderValue::from_str(game_id).expect("Invalid header value"),
        );

        self.get::<CatanMessage>(&url, Some(&headers)).await
    }

    pub async fn send_invite(&self, invite: &Invitation) -> Result<(), ServiceError> {
        let url = "/auth/api/v1/lobby/invite";
        self.post::<&Invitation, ()>(&url, None, Some(&invite))
            .await
    }

    pub async fn invitation_response(
        &self,
        invite: &InvitationResponseData,
    ) -> Result<(), ServiceError> {
        let url = "/auth/api/v1/lobby/acceptinvite";

        self.post::<&InvitationResponseData, ()>(&url, None, Some(invite))
            .await
    }
    // pub async fn start_game(&self, game_id: &str) -> ServiceResponse {
    //     let url = format!("/auth/api/v1/action/start/{}", game_id);
    //     self.post::<()>(&url, None, None).await
    // }

    pub async fn next(&self, game_id: &str) -> Result<Vec<GameAction>, ServiceError> {
        let url = format!("/auth/api/v1/action/next/{}", game_id);
        self.post::<(), Vec<GameAction>>(&url, None, None).await
    }

    pub async fn rotate_login_keys(&self, game_id: &str) -> Result<(), ServiceError> {
        let url = format!("/auth/api/v1/action/start/{}", game_id);
        self.post::<(), ()>(&url, None, None).await
    }

    pub async fn get_all_users(&self) -> Result<Vec<UserProfile>, ServiceError> {
        self.get::<Vec<UserProfile>>("/auth/api/v1/users", None)
            .await
    }

    pub async fn delete_user(&self, user_id: &str) -> Result<(), ServiceError> {
        let url = format!("/auth/api/v1/users/{}", user_id);
        self.delete::<()>(&url, None).await
    }

    pub async fn update_profile(&self, new_profile: &UserProfile) -> Result<(), ServiceError> {
        let url = "/auth/api/v1/users";
        self.put::<&UserProfile, ()>(url, None, Some(new_profile))
            .await
    }
    pub async fn send_phone_code(&self, phone_code: Option<i32>) -> Result<(), ServiceError> {
        let mut headers: HashMap<HeaderName, HeaderValue> = HashMap::new();
        let test_call_context = TestCallContext::new(phone_code, None);
        let json = serde_json::to_string(&test_call_context).unwrap();
        headers.insert(
            HeaderName::from_static(GameHeader::TEST),
            HeaderValue::from_str(&json).expect("Invalid header value"),
        );
        let url = "/auth/api/v1/users/phone/send-code";
        self.post::<(), ()>(url, None, None).await
    }

    pub async fn validate_phone_code(&self, code: i32) -> Result<(), ServiceError> {
        let url = format!("/auth/api/v1/users/phone/validate/{}", code);
        self.post::<(), ()>(&url, None, None).await
    }

    pub async fn send_validation_email(&self) -> Result<String, ServiceError> {
        let url = "/auth/api/v1/users/email/send-validation-email";
        self.post::<(), String>(url, None, None).await
    }

    pub async fn validate_email(&self, token: &str) -> Result<(), ServiceError> {
        let url = format!("/api/v1/users/validate-email/{}", token);
        self.get::<()>(&url, None).await
    }

    pub async fn create_local_user(&self, new_profile: &UserProfile) -> Result<(), ServiceError> {
        let url = "/auth/api/v1/users/local";
        self.post::<&UserProfile, ()>(url, None, Some(new_profile))
            .await
    }
    pub async fn update_local_user(&self, new_profile: &UserProfile) -> Result<(), ServiceError> {
        let url = "/auth/api/v1/users/local";
        self.put::<&UserProfile, ()>(url, None, Some(new_profile))
            .await
    }
    pub async fn delete_local_user(&self, id: &str) -> Result<(), ServiceError> {
        let url = format!("/auth/api/v1/users/local/{}", id);
        self.delete::<()>(&url, None).await
    }
    pub async fn get_local_users(&self, id: &str) -> Result<Vec<UserProfile>, ServiceError> {
        let url = format!("/auth/api/v1/users/local/{}", id);
        self.get::<Vec<UserProfile>>(&url, None).await
    }
}
