/**
 *  this file contains the middleware that injects ServiceContext into the Request.  The data in RequestContext is the
 *  configuration data necessary for the Service to run -- the secrets loaded from the environment, hard coded strings,
 *  etc.
 *
 */
use actix_service::{Service, Transform};
use actix_web::web::Data;
use actix_web::{dev::ServiceRequest, dev::ServiceResponse, Error};
use futures::future::{ok, Ready};
use std::sync::Mutex;
use std::task::{Context, Poll};

use crate::shared::models::CatanSecrets;

/**
 *  Hard coded names for the Database and the Collection. These should be private to this file and only accessed
 *  via RequestContext
 */
const DATABASE_NAME: &'static str = "Users-db";
const COLLECTION_NAME: &'static str = "User-Container";

pub struct ContextMiddleWare;

// This trait is required for middleware (*defined* what it means to be middleware)
impl<S, B> Transform<S, ServiceRequest> for ContextMiddleWare
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = TestFlagMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(TestFlagMiddleware { service })
    }
}

pub struct TestFlagMiddleware<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for TestFlagMiddleware<S>
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
        // fetch is_test flag from the header
        let is_test = req.headers().contains_key("is_test");

        let app_state = req.app_data::<Data<ServiceContext>>().unwrap().clone();
        {
            let mut request_info = app_state.context.lock().unwrap();
            *request_info = RequestContext::create(is_test);
        }

        self.service.call(req)
    }
}

#[derive(Clone)]
pub struct RequestContext {
    pub is_test: bool,
    pub database: String,
    pub collection: String,
    pub secrets: CatanSecrets,
}

impl RequestContext {
    fn new(
        is_test: bool,
        database: String,
        collection: String,
        catan_secrets: &CatanSecrets,
    ) -> Self {
        Self {
            is_test,
            database,
            collection,
            secrets: catan_secrets.clone(),
        }
    }
    pub fn create(is_test: bool) -> Self {
        let (database, collection) = if is_test {
            (
                "user-test-db".to_string(),
                "user-test-collection".to_string(),
            )
        } else {
            (DATABASE_NAME.to_string(), COLLECTION_NAME.to_string())
        };

        let catan_secrets = CatanSecrets::load_from_env().unwrap(); // this panic's if not succesful
        Self {
            is_test,
            database,
            collection,
            secrets: catan_secrets,
        }
    }
}

pub struct ServiceContext {
    pub context: Mutex<RequestContext>,
}

impl ServiceContext {
    pub fn new() -> Self {
        Self {
            context: Mutex::new(RequestContext::new(
                false,
                DATABASE_NAME.to_string(),
                COLLECTION_NAME.to_string(),
                &CatanSecrets::default(),
            )),
        }
    }
}
