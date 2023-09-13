use std::pin::Pin;

use actix::fut::err;
use actix_service::{Service, Transform};
use actix_web::{
    dev::ServiceRequest, dev::ServiceResponse, error::ErrorUnauthorized, Error, HttpMessage,
};

use futures::{
    future::{ok, Ready},
    Future,
};

use super::request_context_mw::RequestContext;

// AuthenticationMiddlewareFactory serves as a factory to create instances of AuthenticationMiddleware
// which is the actual middleware component. It implements the Transform trait required by
// Actix to apply transformations to requests/responses as they pass through the middleware.
pub struct AuthenticationMiddlewareFactory;

// Here, 'static has been added to S. This lifetime specifier is a promise to the Rust compiler
// that any instance of S will live for the entire duration of the program (i.e., it has a static lifetime).
impl<S: 'static, B> Transform<S, ServiceRequest> for AuthenticationMiddlewareFactory
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = AuthenticateMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    // The new_transform method is called when a new Transform is being created.
    // It takes the next service in the middleware chain as an argument, and returns
    // a Future that resolves to either a new Transform (the actual middleware component)
    // or an error.
    fn new_transform(&self, service: S) -> Self::Future {
        ok(AuthenticateMiddleware { service })
    }
}

// AuthenticateMiddleware is the actual middleware component.
// It has a service field that represents the next service in the middleware chain.
pub struct AuthenticateMiddleware<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for AuthenticateMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    // poll_ready is called before every call to check that the service is ready to
    // accept a request. It should return Poll::Ready(Ok(())) when it is ready to
    // accept a request. This macro implements poll_ready
    actix_service::forward_ready!(service);

    // call is invoked for every incoming ServiceRequest.
    // It intercepts each request, checks for the presence and validity of the authorization token,
    // and if the token is missing or invalid, immediately responds with an Unauthorized error.
    // This will also add headers for user_id and email for downstream handlers
    fn call(&self, req: ServiceRequest) -> Self::Future {
        // fetch the authorization header
        let auth_header = req.headers().get("Authorization");

        match auth_header {
            Some(header_value) => {
                let mut request_context = req.extensions_mut().get_mut::<RequestContext>().expect(
                    "You configured a route to use authentication, \
                but not ServiceConfig. This won't work.",
                ).clone();
                let token_str = header_value.to_str().unwrap_or("").replace("Bearer ", "");
                // our token validation logic is predicated on knowing the claim set -- Validation, TestUser, or User
                // but we don't know that until we crack the token to find the claims.  the Test Header will be set
                // by the client to tell us if this is a test, and Validation does not use the auth_mw...so check for
                // the test header, and if is there, use a test claim.  if not, use the normal user claim
                let claims = if request_context.is_test() {
                    request_context
                        .security_context
                        .test_keys
                        .validate_token(&token_str)
                } else {
                    request_context
                        .security_context
                        .login_keys
                        .validate_token(&token_str)
                };

                if claims.is_none() {
                    let fut = err::<ServiceResponse<B>, _>(
                        ErrorUnauthorized("No Authorization Header").into(),
                    );
                    return Box::pin(fut);
                }

                let claims = claims.unwrap();

                request_context.set_claims(&claims);
            }
            None => {
                let fut = err::<ServiceResponse<B>, _>(
                    ErrorUnauthorized("No Authorization Header").into(),
                );
                return Box::pin(fut);
            }
        }
        let fut = self.service.call(req);
        Box::pin(fut)
    }
}
