use crate::shared::models::CatanEnvironmentVariables;
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
use lazy_static::lazy_static;
use std::sync::Mutex;
use std::task::{Context, Poll};

// load the environment variables once and only once the first time they are accessed (which is in main() in this case)
lazy_static! {
    pub static ref CATAN_ENV: CatanEnvironmentVariables =
        CatanEnvironmentVariables::load_from_env().unwrap();
}
/**
 * ContextMiddleWare: This is an implementation of the Transform trait which is required by Actix to define a
 * middleware component. The Transform trait is used to apply transformations to requests/responses as they pass
 * through the middleware. In this case, ContextMiddleWare is used as a factory to create instances of
 * ServiceContextMiddleware which is the actual middleware component.
 */
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
    type Transform = ServiceContextMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(ServiceContextMiddleware { service })
    }
}
/**
 * ServiceContextMiddleware: This struct is your actual middleware component. It has a service field that represents
 * the next service in the middleware chain. This middleware intercepts each request, updates the ServiceContext
 * associated with the request based on the is_test header value, and then passes the request to the next service.
 */
pub struct ServiceContextMiddleware<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for ServiceContextMiddleware<S>
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
/**
 * RequestContext: This struct holds the information that your service needs to operate such as the database,
 * collection, and secrets. This information is stored in a Mutex in the ServiceContext so that it can be safely
 * updated by the middleware during request processing. The is_test flag is used to select between test and
 * production database/collection values.  database_name is stored in the RequestContext so that it can be changed,
 * keeping CatanSecrets read only.
 */
#[derive(Clone)]
pub struct RequestContext {
    pub is_test: bool,
    pub database_name: String,
    pub env: &'static CatanEnvironmentVariables,
}

impl RequestContext {
    fn new(is_test: bool, catan_env: &'static CatanEnvironmentVariables) -> Self {
        let mut db_name: String = catan_env.database_name.clone();
        if is_test {
            db_name += "-test";
        }
        Self {
            database_name: db_name,
            is_test,
            env: catan_env,
        }
    }
    pub fn create(is_test: bool) -> Self {
        return Self::new(is_test, &CATAN_ENV);
    }
}

/**
 * ServiceContext: This struct contains a Mutex<RequestContext> which allows safe, mutable access to the RequestContext
 * from multiple threads. An instance of ServiceContext is created at the start of the application and is stored as
 * shared application data. This ServiceContext is then updated by the ServiceContextMiddleware during each request.
 * "env" is write once, so it is set outside the mutex
 */
pub struct ServiceContext {
    pub context: Mutex<RequestContext>,
    pub env: &'static CatanEnvironmentVariables,
}

impl ServiceContext {
    pub fn new() -> Self {
        Self {
            context: Mutex::new(RequestContext::new(false, &CATAN_ENV)),
            env: &*CATAN_ENV,
        }
    }
}
