#![allow(dead_code)]

use actix_web::http::header::{self, HeaderName, HeaderValue};

use actix_web::test::{self, TestRequest};
use serde::Serialize;
use crate::games_service::catan_games::games::regular::regular_game::RegularGame;
use crate::games_service::game_container::game_messages::{
    GameHeader, Invitation, InvitationResponseData,
};
use crate::games_service::shared::game_enums::CatanGames;
use crate::middleware::request_context_mw::TestContext;
use crate::shared::shared_models::UserProfile;
use crate::shared::shared_models::ServiceResponse;

use actix_http::Request;
use actix_service::Service;
use actix_web::dev::ServiceResponse as ActixServiceResponse;
use actix_web::web::Bytes;
use actix_web::{
    body::{BoxBody, EitherBody},
    Error,
};


use std::collections::HashMap;

pub struct TestProxy<'a, S> {
    test_context: Option<TestContext>,
    service: &'a S,
    auth_token: Option<String>,
}

impl<'a, S> TestProxy<'a, S>
where
    S: Service<Request, Response = ActixServiceResponse<EitherBody<BoxBody>>, Error = Error>,
    S::Future: 'static,
{
    pub fn new(service: &'a S, test_context: Option<TestContext>) -> Self {
        TestProxy {
            test_context,
            service,
            auth_token: None,
        }
    }

    pub fn set_auth_token(&mut self, auth_token: &Option<String>) {
        self.auth_token = auth_token.clone();
    }

    pub fn set_test_context(&mut self, test_context: &Option<TestContext>) {
        self.test_context = test_context.clone();
    }

    pub async fn post<B: Serialize>(
        &self,
        url: &str,
        headers: Option<&HashMap<HeaderName, HeaderValue>>,
        body: Option<B>,
    ) -> ServiceResponse {
        let mut request = TestRequest::post().uri(url);

        // Adding Content-Type header for JSON
        request = request.append_header((header::CONTENT_TYPE, "application/json"));

        if let Some(header_map) = headers {
            for (key, value) in header_map {
                request = request.append_header((key, value));
            }
        }

        if let Some(body_content) = body {
            let bytes = serde_json::to_vec(&body_content).expect("Failed to serialize body");
            request = request.set_payload(Bytes::from(bytes));
        }
        //
        // auth header
        if let Some(auth_token) = &self.auth_token {
            let header_value = format!("Bearer {}", auth_token);
            request = request.append_header(("Authorization", header_value));
        }

        // add the test header
        if let Some(test_context) = &self.test_context {
            let json = serde_json::to_string(&test_context).unwrap();
            request = request.append_header((GameHeader::TEST, json));
        }

        let request = request.to_request();

        let response = test::call_service(self.service, request).await;
        let service_response: ServiceResponse = test::try_read_body_json(response)
            .await
            .expect("should be a ServiceResponse");

        service_response
    }
    pub async fn get(
        &self,
        url: &str,
        headers: Option<&HashMap<HeaderName, HeaderValue>>,
    ) -> ServiceResponse {
        let mut request = TestRequest::get().uri(url);

        if let Some(header_map) = headers {
            for (key, value) in header_map {
                request = request.append_header((key, value));
            }
        }

        // Add the test header
        if let Some(test_context) = &self.test_context {
            let json = serde_json::to_string(test_context).unwrap();
            request = request.append_header((GameHeader::TEST, json));
        }

        if let Some(auth_token) = &self.auth_token {
            let header_value = format!("Bearer {}", auth_token);
            request = request.append_header(("Authorization", header_value));
        }

        let request = request.to_request();

        let response = test::call_service(self.service, request).await;
        let service_response: ServiceResponse = test::try_read_body_json(response)
            .await
            .expect("should be a ServiceResponse");

        service_response
    }
    pub async fn put<B: Serialize>(
        &self,
        url: &str,
        headers: Option<&HashMap<HeaderName, HeaderValue>>,
        body: Option<B>,
    ) -> ServiceResponse {
        let mut request = TestRequest::put().uri(url);  // <-- Use PUT here
    
        // Adding Content-Type header for JSON
        request = request.append_header((header::CONTENT_TYPE, "application/json"));
    
        if let Some(header_map) = headers {
            for (key, value) in header_map {
                request = request.append_header((key, value));
            }
        }
    
        if let Some(body_content) = body {
            let bytes = serde_json::to_vec(&body_content).expect("Failed to serialize body");
            request = request.set_payload(Bytes::from(bytes));
        }
    
        // auth header
        if let Some(auth_token) = &self.auth_token {
            let header_value = format!("Bearer {}", auth_token);
            request = request.append_header(("Authorization", header_value));
        }
    
        // add the test header
        if let Some(test_context) = &self.test_context {
            let json = serde_json::to_string(&test_context).unwrap();
            request = request.append_header((GameHeader::TEST, json));
        }
    
        let request = request.to_request();
    
        let response = test::call_service(self.service, request).await;
        let service_response: ServiceResponse = test::try_read_body_json(response)
            .await
            .expect("should be a ServiceResponse");
    
        service_response
    }
    
   
    pub async fn delete(
        &self,
        url: &str,
        headers: Option<&HashMap<HeaderName, HeaderValue>>,
    ) -> ServiceResponse {
        let mut request = TestRequest::delete().uri(url);

        // Adding Content-Type header for JSON (this might not be necessary for DELETE)
        // but keeping it here for consistency with your POST method
        request = request.append_header((header::CONTENT_TYPE, "application/json"));

        if let Some(header_map) = headers {
            for (key, value) in header_map {
                request = request.append_header((key, value));
            }
        }

        // For DELETE requests, typically there's no body,
        // so I'm excluding the body handling from this method.

        // auth header
        if let Some(auth_token) = &self.auth_token {
            let header_value = format!("Bearer {}", auth_token);
            request = request.append_header(("Authorization", header_value));
        }

        // add the test header
        if let Some(test_context) = &self.test_context {
            let json = serde_json::to_string(&test_context).unwrap();
            request = request.append_header((GameHeader::TEST, json));
        }

        let request = request.to_request();

        let response = test::call_service(self.service, request).await;
        let service_response: ServiceResponse = test::try_read_body_json(response)
            .await
            .expect("should be a ServiceResponse");

        service_response
    }
    pub async fn register(&self, profile: &UserProfile, password: &str) -> ServiceResponse {
        let mut headers: HashMap<HeaderName, HeaderValue> = HashMap::new();

        headers.insert(
            HeaderName::from_static(GameHeader::PASSWORD),
            HeaderValue::from_str(password).expect("Invalid header value"),
        );

        let url = "/api/v1/users/register";
        self.post::<&UserProfile>(url, Some(&headers), Some(profile))
            .await
    }

    pub async fn register_test_user(
        &self,
        profile: &UserProfile,
        password: &str,
    ) -> ServiceResponse {
        let mut headers: HashMap<HeaderName, HeaderValue> = HashMap::new();

        headers.insert(
            HeaderName::from_static(GameHeader::PASSWORD),
            HeaderValue::from_str(password).expect("Invalid header value"),
        );

        let url = "/auth/api/v1/users/register-test-user";
        self.post::<&UserProfile>(url, Some(&headers), Some(profile))
            .await
    }

    pub async fn login(&self, username: &str, password: &str) -> ServiceResponse {
        let mut headers: HashMap<HeaderName, HeaderValue> = HashMap::new();

        headers.insert(
            HeaderName::from_static(GameHeader::PASSWORD),
            HeaderValue::from_str(password).expect("Invalid header value"),
        );
        headers.insert(
            HeaderName::from_static(GameHeader::EMAIL),
            HeaderValue::from_str(username).expect("Invalid header value"),
        );
        let url = "/api/v1/users/login";
        self.post::<()>(url, Some(&headers), None).await
    }
    pub async fn setup(&self) -> ServiceResponse {
        let url = "/api/v1/test/verify-service";
        let sr = self.post::<()>(url, None, None).await;
        sr
    }
    pub async fn get_profile(&self, id: &str) -> ServiceResponse {
        let url = format!("/auth/api/v1/profile/{}", id);

        self.get(&url, None).await
    }

    pub async fn new_game(
        &self,
        game_type: CatanGames,
        game: Option<&RegularGame>,
    ) -> ServiceResponse {
        let url = format!("/auth/api/v1/games/{:?}", game_type);
        let service_response = match game {
            Some(g) => self.post::<&RegularGame>(&url, None, Some(g)).await,
            None => self.post::<()>(&url, None, None).await,
        };

        service_response
    }

    pub async fn get_lobby(&self) -> ServiceResponse {
        let url = "/auth/api/v1/lobby";
        self.get(url, None).await
    }

    pub async fn get_actions(&self, game_id: &str) -> ServiceResponse {
        let url = format!("/auth/api/v1/action/actions/{}", game_id);
        let mut headers: HashMap<HeaderName, HeaderValue> = HashMap::new();

        headers.insert(
            HeaderName::from_static(GameHeader::GAME_ID),
            HeaderValue::from_str(game_id).expect("Invalid header value"),
        );

        self.get(&url, Some(&headers)).await
    }

    pub async fn long_poll(&self, game_id: &str, index: u32) -> ServiceResponse {
        let url = format!("/auth/api/v1/longpoll/{}", index);
        let mut headers: HashMap<HeaderName, HeaderValue> = HashMap::new();

        headers.insert(
            HeaderName::from_static(GameHeader::GAME_ID),
            HeaderValue::from_str(game_id).expect("Invalid header value"),
        );

        self.get(&url, Some(&headers)).await
    }

    pub async fn send_invite(&self, invite: &Invitation) -> ServiceResponse {
        let url = "/auth/api/v1/lobby/invite";
        self.post::<&Invitation>(&url, None, Some(&invite)).await
    }

    pub async fn invitation_response(&self, invite: &InvitationResponseData) -> ServiceResponse {
        let url = "/auth/api/v1/lobby/acceptinvite";

        self.post::<&InvitationResponseData>(&url, None, Some(invite))
            .await
    }
    pub async fn start_game(&self, game_id: &str) -> ServiceResponse {
        let url = format!("/auth/api/v1/action/start/{}", game_id);
        self.post::<()>(&url, None, None).await
    }

    pub async fn next(&self, game_id: &str) -> ServiceResponse {
        let url = format!("/auth/api/v1/action/next/{}", game_id);
        self.post::<&Invitation>(&url, None, None).await
    }

    pub async fn rotate_login_keys(&self, game_id: &str) -> ServiceResponse {
        let url = format!("/auth/api/v1/action/start/{}", game_id);
        self.post::<()>(&url, None, None).await
    }

    pub async fn get_all_users(&self) -> ServiceResponse {
        self.get("/auth/api/v1/users", None).await
    }

    pub async fn delete_user(&self, user_id: &str) -> ServiceResponse {
        let url = format!("/auth/api/v1/users/{}", user_id);
        self.delete(&url, None).await
    }

    pub async fn update_profile(&self, new_profile: &UserProfile) -> ServiceResponse {
        let url = "/auth/api/v1/users";
        self.put::<&UserProfile>(url, None, Some(new_profile))
            .await
    }
    pub async fn send_phone_code(&self) -> ServiceResponse {
        let url= "/auth/api/v1/users/phone/send-code";
        self.post::<()>(url, None, None).await
    }

    pub async fn validate_phone_code(&self, code: i32 ) -> ServiceResponse {
        let url= format!("/auth/api/v1/users/phone/validate/{}", code);
        self.post::<()>(&url, None, None).await
    }

    pub async fn send_validation_email(&self)-> ServiceResponse {
        let url= "/auth/api/v1/users/email/send-validation-email";
        self.post::<()>(url, None, None).await
    }

    pub async fn validate_email(&self, token: &str)-> ServiceResponse {
        let url= format!("/api/v1/users/validate-email/{}", token);
        self.get(&url, None).await
    }

    pub async fn create_local_user(&self, new_profile: &UserProfile) -> ServiceResponse {
        let url = "/auth/api/v1/users/local";
        self.post::<&UserProfile>(url, None, Some(new_profile))
            .await
    }
    pub async fn update_local_user(&self, new_profile: &UserProfile) -> ServiceResponse {
        let url = "/auth/api/v1/users/local";
        self.put::<&UserProfile>(url, None, Some(new_profile))
            .await
    }
    pub async fn delete_local_user(&self, id: &str) -> ServiceResponse {
        let url = format!("/auth/api/v1/users/local/{}", id);
        self.delete(&url, None)
            .await
    }
    pub async fn get_local_users(&self, id: &str) -> ServiceResponse {
        let url = format!("/auth/api/v1/users/local/{}", id);
        self.get(&url, None)
            .await
    }

}

