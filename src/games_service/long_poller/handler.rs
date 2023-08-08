use actix_web::{HttpRequest, HttpResponse};
use azure_core::StatusCode;
use scopeguard::defer;

use crate::{
    full_info,
    games_service::{
        game_container::game_messages::GameHeader, long_poller::long_poller::LongPoller,
    },
    user_service::users::create_http_response,
};

/**
 *  a GET that is a long polling get.  the call waits here until the game changes and then the service will signal
 *  and the call will complete, returning a CatanMessage.  the if GAME_HEADER is missing or "", then we longpoll
 *  for the LOBBY, otherwise send them for game updates.
 */
pub async fn long_poll_handler(req: HttpRequest) -> HttpResponse {
    full_info!("long_poll_handler called");
    defer!(full_info!("long_poll handler exited"));

    let user_id = req
        .headers()
        .get(GameHeader::USER_ID)
        .expect("should be added by auth mw")
        .to_str()
        .unwrap();

    let message = LongPoller::wait(user_id).await;

    match message {
        Ok(message) => HttpResponse::Ok()
            .content_type("application/json")
            .json(message),
        Err(e) => create_http_response(StatusCode::BadRequest, &format!("error: {:#?}", e), ""),
    }
}
