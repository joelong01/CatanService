#![allow(dead_code)]
#![allow(unused_imports)]
use actix_web::{web, HttpRequest, HttpResponse, Responder};
use reqwest::StatusCode;

use crate::{
    games_service::{
        catan_games::traits::game_trait::GameTrait, game_container::game_container::GameContainer,
        shared::game_enums::GameAction,
    },

    user_service::user_handlers::create_http_response,
};

/**
 * this module takes the HTTP requests, calls the appropriate api in action.rs and then constructs the appropriate
 * HTTP response
 */

pub async fn next_handler(game_id: web::Path<String>) -> impl Responder {
    super::actions::next(&game_id).await
    .map(|sr| sr.to_http_response())
    .unwrap_or_else(|sr| sr.to_http_response())
}
/**
 * look at the state of the game and answer the question "what are the valid actions"
 */
pub async fn valid_actions(game_id: web::Path<String>, _req: HttpRequest) -> impl Responder {
    super::actions::valid_actions(&game_id).await
    .map(|sr| sr.to_http_response())
    .unwrap_or_else(|sr| sr.to_http_response())
}
