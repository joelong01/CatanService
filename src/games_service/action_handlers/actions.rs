use actix_web::{web, HttpRequest, Responder, HttpResponse};
use reqwest::StatusCode;

use crate::{games_service::{game_container::game_container::GameContainer, catan_games::traits::game_state_machine_trait::StateMachineTrait, shared::game_enums::GameState}, user_service::users::create_http_response};


pub async fn start_game(game_id_path: web::Path<String>, _req: HttpRequest) -> impl Responder {
    let game_id: &str = &game_id_path;

    let game = match GameContainer::current(&game_id.to_owned()).await {
        Ok(g) => g,
        Err(e) => {
            return create_http_response(
                StatusCode::NOT_FOUND,
                &format!("invalid game_id: {}.  error: {:?}", game_id, e),
                "",
            )
        }
    };
   //if game.next_state(None)
    let mut game_clone = game.clone();
    game_clone.set_current_state(GameState::ChoosingBoard);
    let _ = GameContainer::push_game(game_id, &game_clone).await;
    HttpResponse::Ok().finish()
}
/**
 * look at the state of the game and asnwer the question "what are the valid actions"
 */
pub async fn valid_actions(game_id_path: web::Path<String>, _req: HttpRequest) -> impl Responder {
    let game_id: &str = &game_id_path;

    let game = match GameContainer::current(&game_id.to_owned()).await {
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
        .json(game.state_data.actions())
}
