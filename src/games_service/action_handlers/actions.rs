#![allow(dead_code)]
#![allow(unused_imports)]
use actix_web::{web, HttpRequest, HttpResponse, Responder};
use reqwest::StatusCode;

use crate::{games_service::{
        catan_games::traits::game_trait::GameTrait,
        game_container::game_container::GameContainer,
        shared::game_enums::GameAction,
    }, user_service::user_handlers::create_http_response};

pub async fn next(game_id_path: web::Path<String>, _req: HttpRequest) -> impl Responder {
    let game_id: &str = &game_id_path;

    let (game, can_redo) = match GameContainer::current_game(&game_id.to_owned()).await {
        Ok(g) => g,
        Err(e) => {
            return create_http_response(
                StatusCode::NOT_FOUND,
                &format!("invalid game_id: {}.  error: {:?}", game_id, e),
                "",
            )
        }
    };
    let actions = game.valid_actions(can_redo);
    if !actions.contains(&GameAction::Next) {
        return create_http_response(
            StatusCode::BAD_REQUEST,
            "next not allowed. allowed actions in body.".into(),
            &serde_json::to_string(&actions).expect("serialization should work"),
        );
    }

    // we don't validate if next is ok because next won't be in the list if they can't do it.  we do it this way 
    // so that the client can enable the next button based on the existence of the action...eg if the game doesn't
    // have enough players, we won't give them a "next" action. or if there are unspend entitlements, etc.

    let game_clone = game.set_next_state().unwrap();
    let _ = GameContainer::push_game(game_id, &game_clone).await;
    HttpResponse::Ok().json(game_clone.valid_actions(can_redo))
    
}
/**
 * look at the state of the game and asnwer the question "what are the valid actions"
 */
pub async fn valid_actions(game_id_path: web::Path<String>, _req: HttpRequest) -> impl Responder {
    let game_id: &str = &game_id_path;

    let (game, can_redo) = match GameContainer::current_game(&game_id.to_owned()).await {
        Ok(g) => g,
        Err(e) => {
            return create_http_response(
                StatusCode::NOT_FOUND,
                &format!("invalid game_id: {}.  error: {:?}", game_id, e),
                "",
            )
        }
    };
    HttpResponse::Ok()
        .content_type("application/json")
        .json(game.valid_actions(can_redo))
}
