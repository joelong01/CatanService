#![allow(dead_code)]
#![allow(unused_imports)]
use actix_web::{web, HttpRequest, HttpResponse, Responder};
use reqwest::StatusCode;

use crate::{
    games_service::{
        catan_games::traits::game_trait::GameTrait, game_container::game_container::GameContainer,
        shared::game_enums::GameAction,
    },
    shared::shared_models::{GameError, ResponseType, ServiceError},
};

pub async fn next(game_id: &str) -> Result<Vec<GameAction>, ServiceError> {
    let (game, can_redo) = match GameContainer::current_game(game_id).await {
        Ok(g) => g,
        Err(e) => {
            return Err(ServiceError::new(
                &format!("invalid game id: {}", game_id),
                StatusCode::NOT_FOUND,
                ResponseType::ErrorInfo(format!("{:#?}", e)),
                GameError::HttpError,
            ))
        }
    };
    let actions = game.valid_actions(can_redo);
    if !actions.contains(&GameAction::Next) {
        return Err(ServiceError::new(
            "failed to delete user",
            StatusCode::BAD_REQUEST,
            ResponseType::NoData,
            GameError::HttpError,
        ));
    }

    // we don't validate if next is ok because next won't be in the list if they can't do it.  we do it this way
    // so that the client can enable the next button based on the existence of the action...eg if the game doesn't
    // have enough players, we won't give them a "next" action. or if there are unspend entitlements, etc.

    let game_clone = game.set_next_state().unwrap();
    let _ = GameContainer::push_game(game_id, &game_clone).await;
    Ok(game_clone.valid_actions(can_redo))
}
/**
 * look at the state of the game and answer the question "what are the valid actions"
 */
pub async fn valid_actions(game_id: &str) -> Result<Vec<GameAction>, ServiceError> {
    let (game, can_redo) = match GameContainer::current_game(game_id).await {
        Ok(g) => g,
        Err(e) => {
            return Err(ServiceError::new(
                &format!("invalid game id: {}", game_id),
                StatusCode::NOT_FOUND,
                ResponseType::ErrorInfo(format!("{:#?}", e)),
                GameError::HttpError,
            ))
        }
    };
    Ok(game.valid_actions(can_redo))
}
