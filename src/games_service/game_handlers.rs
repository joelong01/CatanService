use crate::{
    get_header_value,
    shared::header_extractor::HeadersExtractor, middleware::environment_mw::RequestContext,
};
use actix_web::{
    web::{self, Path},
    HttpResponse,
};

use crate::games_service::shared::game_enums::CatanGames;

use super::catan_games::games::regular::regular_game::RegularGame;

///
/// check the state to make sure the request is valid
/// randomize the board and the harbors
/// post the response to websocket
pub async fn shuffle_game(game_id: web::Path<String>) -> HttpResponse {
    super::game::shuffle_game(&game_id)
        .await
        .map(|sr| sr.to_http_response())
        .unwrap_or_else(|sr| sr.to_http_response())
}

///
/// creates a new game and returns a gamedId that is used for all subsequent game* apis.
/// the user header is filled in by the auth middleware.  a JWT token from login must be
/// passed in.  this creates a game and stores it in a global HashMap so that multiple
/// cames can be run at the same time.
pub async fn new_game(
    game_type: Path<CatanGames>,
    headers: HeadersExtractor,
    test_game: Option<web::Json<RegularGame>>,
    request_context: RequestContext
) -> HttpResponse {
    let game_type = game_type.into_inner();
    let user_id = get_header_value!(user_id, headers);
    let test_game: Option<RegularGame> = test_game.map(|json_game| json_game.into_inner());
    super::game::new_game(game_type, &user_id, headers.is_test, test_game, request_context)
        .await
        .map(|sr| sr.to_http_response())
        .unwrap_or_else(|sr| sr.to_http_response())
}

pub async fn supported_games() -> HttpResponse {
    super::game::supported_games()
        .await
        .map(|sr| sr.to_http_response())
        .unwrap_or_else(|sr| sr.to_http_response())
}
