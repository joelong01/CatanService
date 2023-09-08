use actix_web::HttpResponse;


use crate::{
    games_service::long_poller::long_poller::LongPoller, get_header_value,
    shared::header_extractor::HeadersExtractor,
};

/**
 *  a GET that is a long polling get.  the call waits here until the game changes and then the service will signal
 *  and the call will complete, returning a CatanMessage.  the if GAME_HEADER is missing or "", then we longpoll
 *  for the LOBBY, otherwise send them for game updates.
 */
pub async fn long_poll_handler(headers: HeadersExtractor) -> HttpResponse {
    let user_id = get_header_value!(user_id, headers);
    let message = LongPoller::wait(&user_id).await;

    match message {
        Ok(message) => HttpResponse::Ok()
            .content_type("application/json")
            .json(message),
        Err(service_response) => service_response.to_http_response()
    }
}
