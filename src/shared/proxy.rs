#![allow(dead_code)]
use std::collections::HashMap;

use reqwest::{
    header::{self, HeaderName, HeaderValue},
    Client, ClientBuilder, StatusCode,
};
use serde::Serialize;

use url::Url;

use crate::{
    games_service::{
        catan_games::games::regular::regular_game::RegularGame,
        game_container::game_messages::{GameHeader, Invitation, InvitationResponseData},
        shared::game_enums::CatanGames,
    },
    middleware::environment_mw::TestContext,
    shared::models::GameError,
};

use super::models::{ResponseType, ServiceResponse, UserProfile};

pub struct ServiceProxy {
    pub client: Client,
    pub host: Url,
    pub test_context: Option<TestContext>,
}

impl ServiceProxy {
    /// Creates a new Proxy with the specified host
    pub fn new(test_context: Option<TestContext>, host: &str) -> Self {
        let client = ClientBuilder::new()
            .pool_max_idle_per_host(0)
            .build()
            .unwrap();
        Self {
            client,
            host: Url::parse(host).expect(r#"Invalid base URL"#),
            test_context,
        }
    }

    /// Makes a POST request to the specified URL with optional headers and JSON body
    pub async fn post<B: Serialize>(
        &self,
        url: &str,
        headers: impl IntoIterator<Item = (HeaderName, HeaderValue)>,
        body: Option<B>,
    ) -> ServiceResponse {
        let url = match self.host.join(url) {
            Ok(url) => url,
            Err(_) => {
                panic!("Bad URL passed into post: {}", url);
            }
        };

        let mut request_builder = self.client.post(url);

        // Adding Content-Type header for JSON
        request_builder = request_builder.header(header::CONTENT_TYPE, "application/json");

        for (key, value) in headers {
            request_builder = request_builder.header(key, value);
        }

        if let Some(body_content) = body {
            request_builder = request_builder.json(&body_content);
        }

        //
        //  add the test header
        if let Some(test_context) = &self.test_context {
            let json = serde_json::to_string(test_context).unwrap();
            request_builder = request_builder.header(
                HeaderName::from_static(GameHeader::TEST),
                HeaderValue::from_str(&json).expect("valid header value"),
            );
        }

        let result = request_builder.send().await;

        match result {
            Ok(response) => {
                let service_response: ServiceResponse =
                    response.json().await.unwrap_or_else(|_| {
                        // Fallback error response in case JSON parsing fails
                        ServiceResponse::new(
                            "unknown error",
                            StatusCode::INTERNAL_SERVER_ERROR,
                            ResponseType::NoData,
                            GameError::HttpError(StatusCode::INTERNAL_SERVER_ERROR),
                        )
                    });

                service_response
            }
            Err(reqwest_error) => {
                let error_response = ServiceResponse::new(
                    "reqwest error",
                    StatusCode::SERVICE_UNAVAILABLE,
                    ResponseType::ErrorInfo(format!("{:#?}", reqwest_error)),
                    GameError::HttpError(StatusCode::SERVICE_UNAVAILABLE),
                );
                error_response
            }
        }
    }

    /// Makes a GET request to the specified URL with optional headers
    pub async fn get(
        &self,
        url: &str,
        headers: impl IntoIterator<Item = (HeaderName, HeaderValue)>,
    ) -> ServiceResponse {
        let url = match self.host.join(url) {
            Ok(url) => url,
            Err(_) => {
                panic!("Bad URL passed into get: {}", url);
            }
        };
        let mut request_builder = self.client.get(url);
        for (key, value) in headers {
            request_builder = request_builder.header(key, value);
        }
        //
        //  add the test header
        if let Some(test_context) = &self.test_context {
            let json = serde_json::to_string(test_context).unwrap();
            request_builder = request_builder.header(
                HeaderName::from_static(GameHeader::TEST),
                HeaderValue::from_str(&json).expect("valid header value"),
            );
        }
        let response = request_builder.send().await;

        match response {
            Ok(response) => {
                let service_response: ServiceResponse =
                    response.json().await.unwrap_or_else(|_| {
                        // Fallback error response in case JSON parsing fails
                        ServiceResponse::new(
                            "unknown error",
                            StatusCode::INTERNAL_SERVER_ERROR,
                            ResponseType::NoData,
                            GameError::HttpError(StatusCode::INTERNAL_SERVER_ERROR),
                        )
                    });

                service_response
            }
            Err(reqwest_error) => {
                let error_response = ServiceResponse::new(
                    "reqwest error",
                    StatusCode::SERVICE_UNAVAILABLE,
                    ResponseType::ErrorInfo(format!("{:#?}", reqwest_error)),
                    GameError::HttpError(StatusCode::SERVICE_UNAVAILABLE),
                );
                error_response
            }
        }
    }

    pub async fn setup(&self) -> ServiceResponse {
        let headers: HashMap<HeaderName, HeaderValue> = HashMap::new();
        let url = "/api/v1/test/verify_service";
        let sr = self.post::<()>(url, headers, None).await;
        sr
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

    pub async fn get_profile(&self, auth_token: &str) -> ServiceResponse {
        let url = "/auth/api/v1/profile";
        let mut headers: HashMap<HeaderName, HeaderValue> = HashMap::new();

        headers.insert(
            reqwest::header::AUTHORIZATION,
            HeaderValue::from_str(auth_token).expect("Invalid header value"),
        );

        self.get(url, headers).await
    }

    pub async fn new_game(
        &self,
        game_type: CatanGames,
        auth_token: &str,
        game: Option<&RegularGame>,
    ) -> ServiceResponse {
        let url = format!("auth/api/v1/games/{:?}", game_type);

        let mut headers: HashMap<HeaderName, HeaderValue> = HashMap::new();
        headers.insert(
            reqwest::header::AUTHORIZATION,
            HeaderValue::from_str(auth_token).expect("Invalid header value"),
        );

        let service_response = match game {
            Some(g) => self.post::<&RegularGame>(&url, headers, Some(g)).await,
            None => self.post::<()>(&url, headers, None).await,
        };

        service_response
    }

    pub async fn get_lobby(&self, auth_token: &str) -> ServiceResponse {
        let url = "/auth/api/v1/lobby";
        let mut headers: HashMap<HeaderName, HeaderValue> = HashMap::new();
        headers.insert(
            reqwest::header::AUTHORIZATION,
            HeaderValue::from_str(auth_token).expect("Invalid header value"),
        );

        self.get(url, headers).await
    }

    pub async fn get_actions(&self, game_id: &str, auth_token: &str) -> ServiceResponse {
        let url = format!("/auth/api/v1/action/actions/{}", game_id);
        let mut headers: HashMap<HeaderName, HeaderValue> = HashMap::new();
        headers.insert(
            reqwest::header::AUTHORIZATION,
            HeaderValue::from_str(auth_token).expect("Invalid header value"),
        );
        headers.insert(
            HeaderName::from_static(GameHeader::GAME_ID),
            HeaderValue::from_str(game_id).expect("Invalid header value"),
        );

        self.get(&url, headers).await
    }

    pub async fn long_poll(&self, game_id: &str, auth_token: &str, index: u32) -> ServiceResponse {
        let url = format!("/auth/api/v1/longpoll/{}", index);
        let mut headers: HashMap<HeaderName, HeaderValue> = HashMap::new();
        headers.insert(
            reqwest::header::AUTHORIZATION,
            HeaderValue::from_str(auth_token).expect("Invalid header value"),
        );
        headers.insert(
            HeaderName::from_static(GameHeader::GAME_ID),
            HeaderValue::from_str(game_id).expect("Invalid header value"),
        );

        self.get(&url, headers).await
    }

    pub async fn send_invite<'a>(
        &self,
        invite: &'a Invitation,
        auth_token: &'a str,
    ) -> ServiceResponse {
        let url = "/auth/api/v1/lobby/invite";

        let mut headers: HashMap<HeaderName, HeaderValue> = HashMap::new();
        headers.insert(
            reqwest::header::AUTHORIZATION,
            HeaderValue::from_str(auth_token).expect("Invalid header value"),
        );
        headers.insert(
            reqwest::header::CONTENT_TYPE,
            HeaderValue::from_str("application/json").expect("that should be non-controversial"),
        );

        self.post::<&Invitation>(&url, headers, Some(invite)).await
    }
    pub async fn invitation_response<'a>(
        &self,
        invite: &'a InvitationResponseData,
        auth_token: &'a str,
    ) -> ServiceResponse {
        let url = "/auth/api/v1/lobby/acceptinvite";

        let mut headers: HashMap<HeaderName, HeaderValue> = HashMap::new();
        headers.insert(
            reqwest::header::AUTHORIZATION,
            HeaderValue::from_str(auth_token).expect("Invalid header value"),
        );
        headers.insert(
            reqwest::header::CONTENT_TYPE,
            HeaderValue::from_str("application/json").expect("that should be non-controversial"),
        );

        self.post::<&InvitationResponseData>(&url, headers, Some(invite))
            .await
    }

    pub async fn start_game(&self, game_id: &str, auth_token: &str) -> ServiceResponse {
        let url = format!("/auth/api/v1/action/start/{}", game_id);

        let mut headers: HashMap<HeaderName, HeaderValue> = HashMap::new();
        headers.insert(
            reqwest::header::AUTHORIZATION,
            HeaderValue::from_str(auth_token).expect("Invalid header value"),
        );
        headers.insert(
            reqwest::header::CONTENT_TYPE,
            HeaderValue::from_str("application/json").expect("that should be non-controversial"),
        );

        self.post::<()>(&url, headers, None).await
    }

    pub async fn next<'a>(&self, game_id: &str, auth_token: &'a str) -> ServiceResponse {
        let url = format!("/auth/api/v1/action/next/{}", game_id);

        let mut headers: HashMap<HeaderName, HeaderValue> = HashMap::new();
        headers.insert(
            reqwest::header::AUTHORIZATION,
            HeaderValue::from_str(auth_token).expect("Invalid header value"),
        );
        headers.insert(
            reqwest::header::CONTENT_TYPE,
            HeaderValue::from_str("application/json").expect("that should be non-controversial"),
        );

        self.post::<&Invitation>(&url, headers, None).await
    }
}
