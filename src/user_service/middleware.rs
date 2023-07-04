use actix_service::{Service, Transform};
use actix_web::web::Data;
use actix_web::{dev::ServiceRequest, dev::ServiceResponse, Error};
use futures::future::{ok, Ready};
use std::sync::Mutex;
use std::task::{Context, Poll};

use crate::shared::utility::{COLLECTION_NAME, DATABASE_NAME};

pub struct TestFlag;

// This trait is required for middleware
impl<S, B> Transform<S, ServiceRequest> for TestFlag
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
        let (database, collection) = if is_test {
            (
                "user-test-db".to_string(),
                "user-test-collection".to_string(),
            )
        } else {
            (DATABASE_NAME.to_string(), COLLECTION_NAME.to_string())
        };

        let app_state = req.app_data::<Data<AppState>>().unwrap().clone();
        {
            let mut request_info = app_state.request_info.lock().unwrap();
            *request_info = RequestInfo::new(is_test, database, collection);
        }

        self.service.call(req)
    }
}

#[derive(Clone)]
pub struct RequestInfo {
    pub is_test: bool,
    pub database: String,
    pub collection: String,
}

impl RequestInfo {
    fn new(is_test: bool, database: String, collection: String) -> Self {
        Self {
            is_test,
            database,
            collection,
        }
    }
}

pub struct AppState {
    pub request_info: Mutex<RequestInfo>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            request_info: Mutex::new(RequestInfo::new(
                false,
                DATABASE_NAME.to_string(),
                COLLECTION_NAME.to_string(),
            )),
        }
    }
}
