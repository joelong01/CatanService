use crate::{
    api_call,
    middleware::{header_extractor::HeadersExtractor, request_context_mw::RequestContext},
};
use actix_web::{
    web::{self, Path},
    HttpResponse,
};

use crate::games_service::shared::game_enums::CatanGameType;

///
/// check the state to make sure the request is valid
/// randomize the board and the harbors
/// post the response to websocket
pub async fn shuffle_game(
    game_id: web::Path<String>,
    headers: HeadersExtractor,
    request_context: RequestContext,
) -> HttpResponse {
    let test_game = headers.test_call_context.and_then(|ctx| ctx.game.clone());
    api_call!(super::game::shuffle_game(&game_id, &request_context, test_game).await)
}

///
/// creates a new game and returns a gamedId that is used for all subsequent game* apis.
/// the user header is filled in by the auth middleware.  a JWT token from login must be
/// passed in.  this creates a game and stores it in a global HashMap so that multiple
/// cames can be run at the same time.
pub async fn new_game_handler(
    game_type: Path<CatanGameType>,
    headers: HeadersExtractor,
    request_context: RequestContext,
) -> HttpResponse {
    let test_game = headers.test_call_context.and_then(|ctx| ctx.game.clone());

    let game_type = game_type.into_inner();
    let user_id = request_context
        .claims
        .as_ref()
        .expect("if claims can't unwrap, the call should fail in the auth middleware")
        .id
        .clone();

    api_call!(super::game::new_game(game_type, &user_id, test_game, &request_context).await)
}

pub async fn supported_games_handler() -> HttpResponse {
    api_call!(super::game::supported_games().await)
}

pub async fn reload_game_handler(
    game_id: web::Path<String>,
    request_context: RequestContext,
) -> HttpResponse {
    api_call!(super::game::reload_game(&game_id, &request_context).await)
}
