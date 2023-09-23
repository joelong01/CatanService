use actix_web::HttpResponse;

use crate::{
    games_service::long_poller::long_poller::LongPoller, middleware::request_context_mw::RequestContext,
};

/**
 *  a GET that is a long polling get.  the call waits here until the game changes and then the service will signal
 *  and the call will complete, returning a CatanMessage.  the if GAME_HEADER is missing or "", then we longpoll
 *  for the LOBBY, otherwise send them for game updates.
 */
pub async fn long_poll_handler(request_context: RequestContext) -> HttpResponse {
    let user_id = &request_context
        .claims
        .as_ref()
        .expect("auth_mw should set this for all authenticated APIs")
        .id;
    let message = LongPoller::wait(&user_id).await;

    match message {
        Ok(message) => HttpResponse::Ok()
            .content_type("application/json")
            .json(message),
        Err(service_response) => service_response.to_http_response(),
    }
}
