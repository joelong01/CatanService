#![allow(dead_code)]

use crate::cosmos_db::database_abstractions::DatabaseWrapper;
use crate::games_service::catan_games::games::regular::regular_game::RegularGame;
use crate::games_service::game_container::game_messages::GameHeader;
use crate::middleware::service_config::{ServiceConfig, SERVICE_CONFIG};
use crate::shared::service_models::{Claims, Role};
use crate::shared::shared_models::{ProfileStorage, ServiceError, UserProfile};
/**
 *  this file contains the middleware that injects ServiceContext into the Request.  The data in RequestContext is the
 *  configuration data necessary for the Service to run -- the secrets loaded from the environment, hard coded strings,
 *  etc.
 *
 */
use actix_service::{Service, Transform};
use actix_web::dev::Payload;
use actix_web::{dev::ServiceRequest, dev::ServiceResponse, Error};
use actix_web::{FromRequest, HttpMessage, HttpRequest};
use futures::future::{ok, Ready};
use serde::{Deserialize, Serialize};
use std::task::{Context, Poll};

use super::security_context::SecurityContext;

///
/// TestCallContext
/// This struct contains information that is used by tests that apply only to the particular call
/// In order to use this the claims must have is_call_in_role("Test")
#[derive(Serialize, Deserialize, Eq, PartialEq, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TestCallContext {
    pub phone_code: Option<i32>, // send the code to verify phone number so we can test from the client
    pub game: Option<RegularGame>, // send a game from the client so we can test the apis, but know what we get back
}

impl TestCallContext {
    pub fn new(phone_code: Option<i32>, game: Option<RegularGame>) -> Self {
        Self {
            phone_code: phone_code,
            game: game,
        }
    }

    pub fn set_phone_code(&mut self, code: Option<i32>) {
        self.phone_code = code.clone();
    }
    pub fn set_game(&mut self, game: Option<RegularGame>) {
        self.game = game;
    }
}

#[derive(Clone)]
pub struct RequestContext {
    pub config: ServiceConfig,
    pub test_context: Option<TestCallContext>,
    pub claims: Option<Claims>,
    pub security_context: SecurityContext,
}

impl RequestContext {
    pub fn new(
        claims: Option<&Claims>,
        test_call_context: &Option<TestCallContext>,
        service_config: &'static ServiceConfig,
        security_context: &SecurityContext,
    ) -> Self {
       RequestContext {
            config: service_config.clone(), // Clone the read-only environment data
            test_context: test_call_context.clone(),
            claims: claims.cloned(),
            security_context: security_context.clone(),
        }
    }
    pub fn admin_default(profile: &UserProfile) -> Self {
        let test_context = TestCallContext::new(None, None);
        let claims = Claims::new(
            &profile
                .user_id
                .as_ref()
                .map_or(String::default(), String::clone),
            &profile.pii.as_ref().unwrap().email,
            30,
            &vec![Role::TestUser, Role::Admin],
            ProfileStorage::CosmosDb,
        );
     
        let security_context = SecurityContext::cached_secrets().clone();
        Self {
            config: SERVICE_CONFIG.clone(), // Clone the read-only environment data
            test_context: Some(test_context),
            claims: Some(claims),
            security_context: security_context,
        }
    }

    pub fn set_claims(&mut self, claims: &Claims) {
        self.claims = Some(claims.clone());
    }
    pub fn test_default(use_cosmos: bool) -> Self {
        let profile_location = if use_cosmos {
            ProfileStorage::CosmosDbTest
        } else {
            ProfileStorage::MockDb
        };
        RequestContext::new(
            Some(&Claims::new(
                "",
                "",
                60000,
                &vec![Role::TestUser],
                profile_location,
            )),
            &Some(TestCallContext::new(None, None)),
            &SERVICE_CONFIG,
            &SecurityContext::cached_secrets(),
        )
    }

    pub fn is_test(&self) -> bool {
        self.claims
            .as_ref()
            .map_or(false, |claims| claims.roles.contains(&Role::TestUser))
    }



    pub fn database(&self) -> Result<DatabaseWrapper, ServiceError> {
        let location = self
            .claims
            .as_ref()
            .ok_or_else(|| {
                ServiceError::new_bad_request(
                    "need claims to create a db connection from a request context",
                )
            })?
            .profile_storage
            .clone();

        let db = DatabaseWrapper::from_location(location, &SERVICE_CONFIG);
        Ok(db)
    }

    /// Returns the name of the database based on the current context.
    ///
    /// if the profile location is supposed to be the the CosmosDbTest, add -test to the end of the name
    ///
    /// # Returns
    ///
    /// - A `String` representing the name of the database to be used.
    pub fn database_name(&self) -> String {
        if self
            .claims
            .as_ref()
            .map_or(false, |c| c.profile_storage == ProfileStorage::CosmosDbTest)
        {
            format!("{}-test", self.config.cosmos_database_name)
        } else {
            self.config.cosmos_database_name.clone()
        }
    }
    /// Determines if the caller is in the specified role by looking in the claims
    ///
    ///
    /// # Returns
    ///
    /// - A `bool` to indicate if the user is in the role
    pub fn is_caller_in_role(&self, role: Role) -> bool {
        self.claims
            .as_ref()
            .map_or(false, |c| c.roles.contains(&role))
    }
}
impl FromRequest for RequestContext {
    type Error = Error;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        // Fetch the RequestContext from request extensions
        if let Some(request_context) = req.extensions().get::<RequestContext>() {
            ok(request_context.clone()) // Clone the RequestContext
        } else {
            // Handle case where RequestContext is not found  - assume no test
            ok(RequestContext {
                config: SERVICE_CONFIG.clone(), // Clone the environment variables
                test_context: None,
                claims: None, // updated if there are any claims in the auth_mw
                security_context: SecurityContext::cached_secrets(),
            })
        }
    }
}

pub struct RequestContextMiddleware;

impl<S, B> Transform<S, ServiceRequest> for RequestContextMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = RequestContextInjector<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(RequestContextInjector { service })
    }
}

pub struct RequestContextInjector<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for RequestContextInjector<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(&self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        // println!("{:#?}", req.headers());
        // Fetch test context from header
        let test_context = req.headers().get(GameHeader::TEST).and_then(|test_header| {
            test_header
                .to_str()
                .ok()
                .and_then(|value| serde_json::from_str::<TestCallContext>(value).ok())
        });

        // Create RequestContext  - RequestContext runs *before* auth_mw, so claims are always None here
        let request_context = RequestContext::new(
            None,
            &test_context,
            &SERVICE_CONFIG,
            &SecurityContext::cached_secrets(),
        );

        // now we know what database to talk to!

        // Attach the RequestContext to the request's extensions
        req.extensions_mut().insert(request_context);

        self.service.call(req)
    }
}
