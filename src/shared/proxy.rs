#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_imports)]
use std::collections::HashMap;

use actix_http::Method;
use reqwest::{
    header::{self, HeaderName, HeaderValue},
    Client, ClientBuilder, StatusCode,
};
use serde::{de::DeserializeOwned, Serialize};

use url::Url;

use crate::{
    games_service::{
        catan_games::games::regular::regular_game::RegularGame,
        game_container::game_messages::{GameHeader, Invitation, InvitationResponseData, CatanMessage},
        shared::game_enums::{CatanGames, GameAction},
    },
    middleware::request_context_mw::TestContext,
    shared::shared_models::GameError,
};

use super::shared_models::{ResponseType, ServiceError, UserProfile};
///
/// Proxy to the service to make it easier to write tests (or call the service for other reasons)
/// works against the running service -- *not* "test::call_service"
pub struct ServiceProxy {
    test_context: Option<TestContext>,
    service: Client,
    auth_token: Option<String>,
}

impl ServiceProxy {
    /// Creates a new Proxy with the specified host
    pub async fn new(
       service: Client, test_context: Option<TestContext>
    ) -> Result<Self, ServiceError> {
        todo!()
    }

    pub fn new_non_auth(test_context: Option<TestContext>, host: &str) -> Self {
        todo!()
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
        todo!()
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
        profile: &UserProfile,
        password: &str,
    ) -> Result<UserProfile, ServiceError> {
        let mut headers: HashMap<HeaderName, HeaderValue> = HashMap::new();

        headers.insert(
            HeaderName::from_static(GameHeader::PASSWORD),
            HeaderValue::from_str(password).expect("Invalid header value"),
        );

        let url = "/auth/api/v1/users/register-test-user";
        self.post::<&UserProfile, UserProfile>(url, Some(&headers), Some(profile))
            .await
    }

    pub async fn login(&self, username: &str, password: &str) -> Result<String, ServiceError> {
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
        self.post::<(), String>(url, Some(&headers), None).await
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
        game_type: CatanGames,
        game: Option<&RegularGame>,
    ) -> Result<RegularGame, ServiceError> {
        let url = format!("/auth/api/v1/games/{:?}", game_type);
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
    pub async fn send_phone_code(&self) -> Result<(), ServiceError> {
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
