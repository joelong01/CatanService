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

use super::{
    catan_games::{games::regular::regular_game::RegularGame, traits::game_trait::CatanGame},
    game_container::game_container::GameContainer,
};




///
/// check the state to make sure the request is valid
/// randomize the board and the harbors
/// post the response to websocket
pub async fn shuffle_game(game_id_path: web::Path<String>, _req: HttpRequest) -> impl Responder {
    let game_id: &str = &game_id_path;

    match GameContainer::current(&game_id.to_owned()).await {
        Ok(game) => {
            let mut new_game = game.clone();
            new_game.shuffle_count = new_game.shuffle_count + 1;
            new_game.shuffle();
            GameContainer::push_game(&game_id.to_owned(), &new_game).await;
            HttpResponse::Ok()
            .content_type("application/json")
            .json(game)
        }
        Err(_e)=>{
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
        }
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
    let player_id = req.headers().get("user_id").unwrap().to_str().unwrap();

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

    let user_result = internal_find_user("id".to_owned(), player_id.to_owned(), data).await;

    let user = match user_result {
        Ok(u) => u,
        Err(_) => {
            return create_http_response(
                StatusCode::NotFound,
                format!("invalid user id: {}", player_id),
                "".to_owned(),
            );
        }
    };

    let mut game = RegularGame::new(&ClientUser::from_persist_user(user));
    game.shuffle();

    //
    //  store GameId --> Game for later lookup

    GameContainer::insert_container(player_id.to_owned(), game.id.to_owned(), &mut game).await;

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
