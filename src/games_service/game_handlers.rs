use crate::{
    api_call, full_info,
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
    if let Some(g) = &test_game {
        full_info!("baron_tile: {:#?}", g.baron_tile);
    }
    let result = super::game::shuffle_game(&game_id, &request_context, test_game.clone()).await;
    match result {
        Ok(val) => {
            if let Some(game_clone) = &test_game {
                let json1 = serde_json::to_string(&game_clone).unwrap();
                let json2 = serde_json::to_string(&val).unwrap();

                if json1 != json2 {
                    full_info!("games are different");
                } else {
                    full_info!("games are the same");
                }
            } else {
                full_info!("No test_game provided");
            }

            HttpResponse::Ok().json(&val)
        }
        Err(service_error) => service_error.to_http_response(),
    }
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
