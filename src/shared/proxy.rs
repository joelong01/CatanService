#![allow(dead_code)]
use std::collections::HashMap;

use futures::Future;
use reqwest::{
    header::{self, HeaderName, HeaderValue},
    Client, ClientBuilder, Response,
};
use serde::Serialize;
use url::Url;

use crate::games_service::{game_container::game_messages::GameHeaders, shared::game_enums::CatanGames};

use super::models::UserProfile;

pub struct ServiceProxy {
    pub client: Client,
    pub host: Url,
    pub is_test: bool,
}

impl ServiceProxy {
    /// Creates a new Proxy with the specified host
    pub fn new(is_test: bool, host: &str) -> Self {
        let client = ClientBuilder::new()
            .pool_max_idle_per_host(0)
            .build()
            .unwrap();
        Self {
            client,
            host: Url::parse(host).expect(r#"Invalid base URL"#),
            is_test,
        }
    }

    /// Makes a POST request to the specified URL with optional headers and JSON body
    pub fn post<B: Serialize>(
        &self,
        url: &str,
        headers: impl IntoIterator<Item = (HeaderName, HeaderValue)>,
        body: Option<B>,
    ) -> impl Future<Output = Result<Response, reqwest::Error>> {
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

        let response = request_builder.send();
        response
    }

    /// Makes a GET request to the specified URL with optional headers
    pub fn get(
        &self,
        url: &str,
        headers: impl IntoIterator<Item = (HeaderName, HeaderValue)>,
    ) -> impl Future<Output = Result<Response, reqwest::Error>> {
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
        let response = request_builder.send();
        response
    }

    pub fn setup(&self) -> impl Future<Output = Result<Response, reqwest::Error>> {
        let mut headers: HashMap<HeaderName, HeaderValue> = HashMap::new();
        headers.insert(
            HeaderName::from_static(GameHeaders::IS_TEST),
            HeaderValue::from_static("true"),
        );
        let url = "/api/v1/test/setup";
        self.post::<()>(url, headers, None)
    }

    pub fn register<'a>(
        &self,
        profile: &'a UserProfile,
        password: &'a str,
    ) -> impl Future<Output = Result<Response, reqwest::Error>> + 'a {
        let mut headers: HashMap<HeaderName, HeaderValue> = HashMap::new();
        if self.is_test {
            headers.insert(
                HeaderName::from_static(GameHeaders::IS_TEST),
                HeaderValue::from_static("true"),
            );
        }
        headers.insert(
            HeaderName::from_static(GameHeaders::PASSWORD),
            HeaderValue::from_str(password).expect("Invalid header value"),
        );

        let url = "/api/v1/users/register";
        self.post::<&UserProfile>(url, headers, Some(profile))
    }

    pub fn login(
        &self,
        username: &str,
        password: &str,
    ) -> impl Future<Output = Result<Response, reqwest::Error>> {
        let mut headers: HashMap<HeaderName, HeaderValue> = HashMap::new();
        if self.is_test {
            headers.insert(
                HeaderName::from_static(GameHeaders::IS_TEST),
                HeaderValue::from_static("true"),
            );
        }
        headers.insert(
            HeaderName::from_static(GameHeaders::PASSWORD),
            HeaderValue::from_str(password).expect("Invalid header value"),
        );
        headers.insert(
            HeaderName::from_static(GameHeaders::EMAIL),
            HeaderValue::from_str(username).expect("Invalid header value"),
        );
        let url = "/api/v1/users/login";
        self.post::<()>(url, headers, None)
    }

    pub fn get_profile(
        &self,
        auth_token: &str,
    ) -> impl Future<Output = Result<Response, reqwest::Error>> {
        let url = "/auth/api/v1/profile";
        let mut headers: HashMap<HeaderName, HeaderValue> = HashMap::new();
        if self.is_test {
            headers.insert(
                HeaderName::from_static(GameHeaders::IS_TEST),
                HeaderValue::from_static("true"),
            );
        }
        headers.insert(
            reqwest::header::AUTHORIZATION,
            HeaderValue::from_str(auth_token).expect("Invalid header value"),
        );

        self.get(url, headers)
    }

    pub fn new_game(
        &self,
        game_type: CatanGames,
        auth_token: &str,
    ) -> impl Future<Output = Result<Response, reqwest::Error>> {
        let url = format!("auth/api/v1/games/{:?}", game_type);

        
        let mut headers: HashMap<HeaderName, HeaderValue> = HashMap::new();
        if self.is_test {
            headers.insert(
                HeaderName::from_static(GameHeaders::IS_TEST),
                HeaderValue::from_static("true"),
            );
        }
        headers.insert(
            reqwest::header::AUTHORIZATION,
            HeaderValue::from_str(auth_token).expect("Invalid header value"),
        );

        self.post::<()>(&url, headers, None)
    }

    pub fn get_lobby(
        &self,
        auth_token: &str,
    ) -> impl Future<Output = Result<Response, reqwest::Error>> {
        let url = "/auth/api/v1/lobby";
        let mut headers: HashMap<HeaderName, HeaderValue> = HashMap::new();
        if self.is_test {
            headers.insert(
                HeaderName::from_static(GameHeaders::IS_TEST),
                HeaderValue::from_static("true"),
            );
        }
        headers.insert(
            reqwest::header::AUTHORIZATION,
            HeaderValue::from_str(auth_token).expect("Invalid header value"),
        );

        self.get(url, headers)
    }

    pub fn long_poll(
        &self,
        game_id: &str,
        auth_token: &str,
    ) -> impl Future<Output = Result<Response, reqwest::Error>> {
        let url = "/auth/api/v1/longpoll";
        let mut headers: HashMap<HeaderName, HeaderValue> = HashMap::new();
        if self.is_test {
            headers.insert(
                HeaderName::from_static(GameHeaders::IS_TEST),
                HeaderValue::from_static("true"),
            );
        }
        headers.insert(
            reqwest::header::AUTHORIZATION,
            HeaderValue::from_str(auth_token).expect("Invalid header value"),
        );
        headers.insert(
            HeaderName::from_static(GameHeaders::GAME_ID),
            HeaderValue::from_str(game_id).expect("Invalid header value")
        );

        self.get(url, headers)
    }
}
