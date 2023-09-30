#![allow(dead_code)]

use crate::cosmos_db::database_abstractions::DatabaseWrapper;
use crate::games_service::catan_games::games::regular::regular_game::RegularGame;
use crate::games_service::game_container::game_messages::GameHeader;
use crate::middleware::service_config::{ServiceConfig, SERVICE_CONFIG};
use crate::shared::service_models::{Claims, Role};
use crate::shared::shared_models::UserProfile;
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

#[derive(Serialize, Deserialize, Eq, PartialEq, Debug, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct TestContext {
    pub use_cosmos_db: bool,
    pub phone_code: Option<i32>, // send the code to verify phone number so we can test from the client
    pub game: Option<RegularGame>, // send a game from the client so we can test the apis, but know what we get back
}

impl TestContext {
    pub fn new(use_cosmos_db: bool, phone_code: Option<i32>, game: Option<RegularGame>) -> Self {
        Self {
            use_cosmos_db,
            phone_code: phone_code,
            game: game,
        }
    }
    pub fn as_json(use_cosmos: bool) -> String {
        let tc = TestContext::new(use_cosmos, None, None);
        serde_json::to_string(&tc).unwrap()
    }
    pub fn set_phone_code(&mut self, code: Option<i32>) {
        self.phone_code = code.clone();
    }
    pub fn set_galme(&mut self, game: Option<RegularGame>) {
        self.game = game;
    }
}

pub struct RequestContext {
    pub config: ServiceConfig,
    pub test_context: Option<TestContext>,
    pub database: Box<DatabaseWrapper>,
    pub claims: Option<Claims>,
    pub security_context: SecurityContext,
}

impl Clone for RequestContext {
    fn clone(&self) -> Self {
        log::trace!("Cloning Request Context");
        RequestContext::new(
            &self.claims,
            &self.test_context,
            &SERVICE_CONFIG,
            &self.security_context,
        )
    }
}

impl RequestContext {
    pub fn new(
        claims: &Option<Claims>,
        test_context: &Option<TestContext>,
        service_config: &'static ServiceConfig,
        security_context: &SecurityContext,
    ) -> Self {
        let database_wrapper = DatabaseWrapper::new(test_context, service_config);

        RequestContext {
            config: service_config.clone(), // Clone the read-only environment data
            test_context: test_context.clone(),

            database: Box::new(database_wrapper),
            claims: claims.clone(),
            security_context: security_context.clone(),
        }
    }

    pub fn admin_default(use_cosmos: bool, profile: &UserProfile) -> Self {
        let test_context = TestContext::new(use_cosmos, None, None);
        let claims = Claims::new(
            &profile.user_id.as_ref().unwrap(),
            &profile.pii.as_ref().unwrap().email,
            30,
            &vec![Role::TestUser, Role::Admin],
            &Some(test_context.clone()),
        );
        let database_wrapper = DatabaseWrapper::new(&Some(test_context.clone()), &SERVICE_CONFIG);

        let security_context = SecurityContext::cached_secrets().clone();
        Self {
            config: SERVICE_CONFIG.clone(), // Clone the read-only environment data
            test_context: Some(test_context.clone()),

            database: Box::new(database_wrapper),
            claims: Some(claims.clone()),
            security_context: security_context,
        }
    }

    pub fn set_claims(&mut self, claims: &Claims) {
        self.claims = Some(claims.clone());
    }
    pub fn test_default(use_cosmos: bool) -> Self {
        RequestContext::new(
            &None,
            &Some(TestContext::new(use_cosmos, None, None)),
            &SERVICE_CONFIG,
            &SecurityContext::cached_secrets(),
        )
    }

    pub fn is_test(&self) -> bool {
        self.test_context.is_some()
    }

    pub fn use_mock_db(&self) -> bool {
        match self.test_context.clone() {
            Some(ctx) => !ctx.use_cosmos_db,
            None => false,
        }
    }

    pub fn use_cosmos_db(&self) -> bool {
        match self.test_context.clone() {
            Some(b) => b.use_cosmos_db,
            None => true,
        }
    }

    pub fn database_name(&self) -> String {
        match self.test_context.clone() {
            Some(_) => format!("{}-test", self.config.cosmos_database_name),
            None => self.config.cosmos_database_name.clone(),
        }
    }

    pub fn is_caller_in_role(&self, role: Role) -> bool {
        match self.claims.clone() {
            Some(c) => c.roles.contains(&role),
            None => false,
        }
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
                database: Box::new(DatabaseWrapper::new(&None, &SERVICE_CONFIG)),
                claims: None,
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
                .and_then(|value| serde_json::from_str::<TestContext>(value).ok())
        });

        // Create RequestContext  - RequestContext runs *before* auth_mw, so claims are always None here
        let request_context = RequestContext::new(
            &None,
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
