use crate::shared::{models::ServiceResponse, utility::get_id};
use actix_web::{web::Path, HttpResponse, Responder};
use azure_core::StatusCode;

use crate::shared::models::{CatanGames, GameData, SupportedGames};

pub async fn new_game(game_type: Path<CatanGames>) -> impl Responder {
    let game_type = game_type.into_inner();
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
    let game_data = GameData { id: get_id() };
    HttpResponse::Ok()
        .content_type("application/json")
        .json(game_data)
}

pub async fn supported_games() -> impl Responder {
    let games = SupportedGames {
        catan_games: vec![CatanGames::Regular],
    };
    HttpResponse::Ok()
        .content_type("application/json")
        .json(games)
}
