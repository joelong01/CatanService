#![allow(dead_code)]
use crate::cosmos_db::cosmosdb::{UserDb, UserDbTrait};
use crate::cosmos_db::mocked_db::TestDb;
use crate::games_service::game_container::game_messages::GameHeader;
use crate::shared::service_models::{Claims, Role};
use crate::middleware::service_config::{ServiceConfig, SERVICE_CONFIG};
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

#[derive(Serialize, Deserialize, Eq, PartialEq, Debug, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct TestContext {
    pub use_cosmos_db: bool,
}

impl TestContext {
    pub fn new(use_cosmos_db: bool) -> Self {
        Self { use_cosmos_db }
    }
    pub fn as_json(use_cosmos: bool) -> String {
        let tc = TestContext {
            use_cosmos_db: use_cosmos,
        };
        serde_json::to_string(&tc).unwrap()
    }
}

pub struct RequestContext {
    pub config: ServiceConfig,
    pub test_context: Option<TestContext>,
    pub database: Box<dyn UserDbTrait>,
    pub claims: Option<Claims>
}

impl Clone for RequestContext {
    fn clone(&self) -> Self {
        log::trace!("Cloning Request Context");
        RequestContext::new(&self.claims, &self.test_context, &SERVICE_CONFIG) 
    }
}


impl RequestContext {
    pub fn new(
        claims: &Option<Claims>,
        test_context: &Option<TestContext>,
        service_config: &'static ServiceConfig,
    ) -> Self {

        let database: Box<dyn UserDbTrait> = match test_context {
            Some(context) => {
                if context.use_cosmos_db {
                    Box::new(UserDb::new(true, service_config))
                } else {
                    Box::new(TestDb::new())
                }
            }
            None => Box::new(UserDb::new(false, service_config)),
        };
        RequestContext {
            config: service_config.clone(), // Clone the read-only environment data
            test_context: test_context.clone(),
            database,
            claims: claims.clone()
        }
    }
    pub fn set_claims(&mut self, claims: &Claims) {
        self.claims = Some(claims.clone());
    }
    pub fn test_default(use_cosmos: bool) -> Self {
        RequestContext::new(&None, &Some(TestContext::new(use_cosmos)), &SERVICE_CONFIG)
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
            None => false
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
                database: Box::new(UserDb::new(false, &SERVICE_CONFIG)), 
                claims:None
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
        // Fetch test context from header
        let test_context = req.headers().get(GameHeader::TEST).and_then(|test_header| {
            test_header
                .to_str()
                .ok()
                .and_then(|value| serde_json::from_str::<TestContext>(value).ok())
        });

        

        // Create RequestContext  - RequestContext runs *before* auth_mw, so claims are always None here
        let request_context = RequestContext::new(&None, &test_context, &SERVICE_CONFIG);

        // now we know what database to talk to!

        // Attach the RequestContext to the request's extensions
        req.extensions_mut().insert(request_context);

        self.service.call(req)
    }
}
