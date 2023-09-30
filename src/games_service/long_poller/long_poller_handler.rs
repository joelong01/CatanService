use actix_web::HttpResponse;

use crate::{
    api_call, games_service::long_poller::long_poller::LongPoller,
    middleware::request_context_mw::RequestContext,
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

    api_call!(LongPoller::wait(&user_id).await)
}
