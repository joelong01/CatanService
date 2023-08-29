#![allow(dead_code)]
use crate::games_service::game_container::game_messages::GameHeader;
use crate::shared::models::ConfigEnvironmentVariables;
/**
 *  this file contains the middleware that injects ServiceContext into the Request.  The data in RequestContext is the
 *  configuration data necessary for the Service to run -- the secrets loaded from the environment, hard coded strings,
 *  etc.
 *
 */
use actix_service::{Service, Transform};
use actix_web::dev::Payload;
use actix_web::{HttpMessage, FromRequest, HttpRequest};
use actix_web::{dev::ServiceRequest, dev::ServiceResponse, Error};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::task::{Context, Poll};
use futures::future::{ok, Ready};


#[derive(Serialize, Deserialize, Eq, PartialEq, Debug, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct TestContext {
    pub use_cosmos_db: bool,
}


impl TestContext {
    pub fn new(use_cosmos_db: bool) -> Self {
        Self {
            use_cosmos_db,
        }
    }
    pub fn as_json(use_cosmos: bool) -> String {
        let tc = TestContext {
            use_cosmos_db: use_cosmos,
        };
        serde_json::to_string(&tc).unwrap()
    }
}
#[derive(Clone)]
pub struct RequestContext {
    pub env: ConfigEnvironmentVariables,
    pub test_context: Option<TestContext>,
}

impl RequestContext {
    pub fn new(
        test_context: Option<TestContext>,
        catan_env: &'static ConfigEnvironmentVariables,
    ) -> Self {
        RequestContext {
            env: catan_env.clone(), // Clone the read-only environment data
            test_context,
        }
    }

    pub fn test_default(use_cosmos: bool) -> Self {
        RequestContext::new(Some(TestContext::new(use_cosmos)), &CATAN_ENV)
    }

    pub fn is_test(&self) -> bool {
        self.test_context.is_some()
    }

    pub fn use_mock_db(&self) -> bool {
        match self.test_context.clone() {
            Some(ctx) => !ctx.use_cosmos_db,
            None => false
        }
    }

    pub fn database_name(&self) -> String {
        match self.test_context.clone() {
            Some(_) => format!("{}-test", self.env.database_name),
            None => self.env.database_name.clone()
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
                env: CATAN_ENV.clone(), // Clone the environment variables
                test_context: None,
            })
        }
    }
}


// load the environment variables once and only once the first time they are accessed (which is in main() in this case)
lazy_static! {
    pub static ref CATAN_ENV: ConfigEnvironmentVariables =
        ConfigEnvironmentVariables::load_from_env().unwrap();
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
    
        // Create RequestContext by combining environment and test context
        let request_context = RequestContext::new(test_context, &CATAN_ENV);
    
        // Attach the RequestContext to the request's extensions
        req.extensions_mut().insert(request_context);
    
        self.service.call(req)
    }
    
}
