#![allow(unused_imports)]
#![allow(dead_code)]

use actix_web::http::header::{self, HeaderName, HeaderValue};

use actix_web::test::{self, TestRequest};

use azure_core::auth;
use serde::Serialize;

use crate::games_service::catan_games::games::regular::regular_game::RegularGame;
use crate::games_service::game_container::game_messages::GameHeader;
use crate::games_service::shared::game_enums::CatanGames;
use crate::middleware::request_context_mw::TestContext;
use crate::shared::shared_models::{self, UserProfile};
use crate::{create_app, shared::shared_models::ServiceResponse};

use actix_http::Request;
use actix_service::Service;
use actix_web::dev::ServiceResponse as ActixServiceResponse;
use actix_web::web::Bytes;
use actix_web::{
    body::{BoxBody, EitherBody},
    Error,
};
use futures::Future;
use std::collections::HashMap;
use std::pin::Pin;
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

    pub fn set_auth_token(&mut self, auth_token: &str) {
        self.auth_token = Some(auth_token.to_owned());
    }

    // Now we can modify the post function to accept the service as an argument:

    pub async fn post<B: Serialize>(
        &self,
        url: &str,
        headers: impl IntoIterator<Item = (HeaderName, HeaderValue)>,
        body: Option<B>,
    ) -> ServiceResponse {
        let mut request = TestRequest::post().uri(url);

        // Adding Content-Type header for JSON
        request = request.append_header((header::CONTENT_TYPE, "application/json"));

        for (key, value) in headers {
            request = request.append_header((key, value));
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
        headers: impl IntoIterator<Item = (HeaderName, HeaderValue)>,
    ) -> ServiceResponse
where {
        let mut request = TestRequest::get().uri(url);

        // Process headers
        for (key, value) in headers {
            request = request.append_header((key, value));
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
    pub async fn register(&self, profile: &UserProfile, password: &str) -> ServiceResponse {
        let mut headers: HashMap<HeaderName, HeaderValue> = HashMap::new();

        headers.insert(
            HeaderName::from_static(GameHeader::PASSWORD),
            HeaderValue::from_str(password).expect("Invalid header value"),
        );

        let url = "/api/v1/users/register";
        self.post::<&UserProfile>(url, headers, Some(profile)).await
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
        self.post::<()>(url, headers, None).await
    }
    pub async fn setup(&self) -> ServiceResponse {
        let headers: HashMap<HeaderName, HeaderValue> = HashMap::new();
        let url = "/api/v1/test/verify-service";
        let sr = self.post::<()>(url, headers, None).await;
        sr
    }
    pub async fn get_profile(&self) -> ServiceResponse {
        let url = "/auth/api/v1/profile";
        let headers: HashMap<HeaderName, HeaderValue> = HashMap::new();
        self.get(url, headers).await
    }

    pub async fn new_game(
        &self,
        game_type: CatanGames,
        game: Option<&RegularGame>,
    ) -> ServiceResponse {
        let url = format!("auth/api/v1/games/{:?}", game_type);
        let service_response = match game {
            Some(g) => self.post::<&RegularGame>(&url, HashMap::new(), Some(g)).await,
            None => self.post::<()>(&url, HashMap::new(), None).await,
        };

        service_response
    }

    pub async fn get_lobby(&self) -> ServiceResponse {
        let url = "/auth/api/v1/lobby";
        self.get(url, HashMap::new()).await
    }

    pub async fn get_actions(&self, game_id: &str) -> ServiceResponse {
        let url = format!("/auth/api/v1/action/actions/{}", game_id);
        let mut headers: HashMap<HeaderName, HeaderValue> = HashMap::new();

        headers.insert(
            HeaderName::from_static(GameHeader::GAME_ID),
            HeaderValue::from_str(game_id).expect("Invalid header value"),
        );

        self.get(&url, headers).await
    }

    pub async fn long_poll(&self, game_id: &str, index: u32) -> ServiceResponse {
        let url = format!("/auth/api/v1/longpoll/{}", index);
        let mut headers: HashMap<HeaderName, HeaderValue> = HashMap::new();

        headers.insert(
            HeaderName::from_static(GameHeader::GAME_ID),
            HeaderValue::from_str(game_id).expect("Invalid header value"),
        );

        self.get(&url, headers).await
    }
}
