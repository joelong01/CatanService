use crate::{
    middleware::environment_mw::ServiceEnvironmentContext,
    shared::models::{ClientUser, ServiceResponse},
    user_service::users::{create_http_response, internal_find_user},
};
use actix_web::{
    web::{self, Data, Path},
    HttpRequest, HttpResponse, Responder,
};
use azure_core::StatusCode;

use crate::games_service::shared::game_enums::{CatanGames, SupportedGames};

use super::catan_games::{
    games::regular::regular_game::RegularGame, traits::game_trait::CatanGame,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

// Define a global HashMap wrapped in Arc<Mutex>
lazy_static::lazy_static! {
    static ref GAME_MAP: Arc<Mutex<HashMap<String, RegularGame>>> = Arc::new(Mutex::new(HashMap::new()));
}
///
/// check the state to make sure the request is valid
/// randomize the board and the harbors
/// post the response to websocket
pub async fn shuffle_game(
    game_id: web::Path<String>,
    req: HttpRequest,
) -> impl Responder {
    let mut game_map = GAME_MAP.lock().unwrap();

    let game = match game_map.get_mut(game_id.as_str()) {
        Some(game) => game,
        None => {
            let response = ServiceResponse {
                message: format!("Bad gameId"),
                status: StatusCode::BadRequest,
                body: "".to_owned(),
            };
            return HttpResponse::BadRequest()
                .content_type("application/json")
                .json(response);
        }
    };

    let user_id = req
        .headers()
        .get("user_id")
        .and_then(|header| header.to_str().ok())
        .unwrap_or_default();
    if user_id != game.creator_id {
        let response = ServiceResponse {
            message: format!(
                "Only the creator can shuffle the board, and you are not the creator."
            ),
            status: StatusCode::BadRequest,
            body: "".to_owned(),
        };
        return HttpResponse::BadRequest()
            .content_type("application/json")
            .json(response);
    }

    game.shuffle();

    HttpResponse::Ok()
        .content_type("application/json")
        .json(game)
}

///
/// creates a new game and returns a gamedId that is used for all subsequent game* apis.
/// the user header is filled in by the auth middleware.  a JWT token from login must be
/// passed in.  this creates a game and stores it in a global HashMap so that multiple
/// cames can be run at the same time.
pub async fn new_game(
    game_type: Path<CatanGames>,
    data: Data<ServiceEnvironmentContext>,
    req: HttpRequest,
) -> impl Responder {
    let game_type = game_type.into_inner();
    let user_id = req.headers().get("user_id").unwrap().to_str().unwrap();

    if game_type != CatanGames::Regular {
        let response = ServiceResponse {
            message: format!("Game not supported: {:#?}", game_type),
            status: StatusCode::BadRequest,
            body: "".to_owned(),
        };
        return HttpResponse::BadRequest()
            .content_type("application/json")
            .json(response);
    }

    let user_result = internal_find_user("id".to_owned(), user_id.to_owned(), data).await;

    let user = match user_result {
        Ok(u) => u,
        Err(_) => {
            return create_http_response(
                StatusCode::NotFound,
                format!("invalid user id: {}", user_id),
                "".to_owned(),
            );
        }
    };

    let mut game = RegularGame::new(&ClientUser::from_persist_user(user));
    game.shuffle();

    //
    //  store GameId --> Game for later lookup
    let mut game_map = GAME_MAP.lock().unwrap();
    game_map.insert(game.id.clone(), game.clone());

    HttpResponse::Ok()
        .content_type("application/json")
        .json(game)
}

pub async fn supported_games() -> impl Responder {
    let games = SupportedGames {
        catan_games: vec![CatanGames::Regular],
    };
    HttpResponse::Ok()
        .content_type("application/json")
        .json(games)
}
