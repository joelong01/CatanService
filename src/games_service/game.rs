use crate::{
    full_info,
    games_service::{
        game_container::game_messages::{CatanMessage, GameCreatedData},
        long_poller::long_poller::LongPoller,
    },
    middleware::request_context_mw::RequestContext,
    shared::shared_models::{GameError, ResponseType, ServiceError, UserProfile},
};

use reqwest::StatusCode;

use crate::games_service::shared::game_enums::CatanGameType;

use super::{
    catan_games::{games::regular::regular_game::RegularGame, traits::game_trait::GameTrait},
    game_container::game_container::GameContainer,
};

///
/// check the state to make sure the request is valid
/// randomize the board and the harbors
/// post the response to websocket
pub async fn shuffle_game(
    game_id: &str,
    request_context: &RequestContext,
) -> Result<RegularGame, ServiceError> {
    if request_context.is_test() {
        full_info!("test shuffle");
    }

    let (game, _) = GameContainer::current_game(&game_id.to_owned()).await?;

    let mut new_game = game.clone();
    new_game.shuffle_count = new_game.shuffle_count + 1;
    new_game.shuffle();
    let result = GameContainer::push_game(&game_id.to_owned(), &new_game).await;
    match result {
        Ok(_) => Ok(new_game),
        Err(e) => {
            let err_message = format!("GameContainer::push_game error: {:#?}", e);
            return Err(ServiceError::new(
                "Error Hashing Password",
                StatusCode::INTERNAL_SERVER_ERROR,
                ResponseType::ErrorInfo(err_message.to_owned()),
                GameError::HttpError,
            ));
        }
    }
}

///
/// creates a new game and returns a gamedId that is used for all subsequent game* apis.
/// the user header is filled in by the auth middleware.  a JWT token from login must be
/// passed in.  this creates a game and stores it in a global HashMap so that multiple
/// cames can be run at the same time.
pub async fn new_game(
    game_type: CatanGameType,
    user_id: &str,
    test_game: Option<RegularGame>,
    request_context: &RequestContext,
) -> Result<RegularGame, ServiceError> {
    if game_type != CatanGameType::Regular {
        return Err(ServiceError::new_unsupported_game(game_type));
    }

    let user = request_context
        .database
        .as_user_db()
        .find_user_by_id(user_id)
        .await?;

    let game = match test_game {
        Some(game) => game,
        None => {
            let mut new_game = RegularGame::new(&UserProfile::from_persist_user(&user));
            new_game.shuffle();
            new_game
        }
    };

    GameContainer::create_and_add_container(&game.game_id, &game, &request_context).await?;

    // Send a message to the user that the game was created
    let _ = LongPoller::send_message(
        vec![user_id.to_string()],
        &CatanMessage::GameCreated(GameCreatedData {
            user_id: user_id.to_string(),
            game_id: game.game_id.to_string(),
        }),
    )
    .await;

    Ok(game)
}

pub async fn supported_games() -> Result<Vec<CatanGameType>, ServiceError> {
    Ok(vec![CatanGameType::Regular])
}

///
/// check the state to make sure the request is valid
/// randomize the board and the harbors
/// post the response to websocket
pub async fn reload_game(
    game_id: &str,
    request_context: &RequestContext,
) -> Result<RegularGame, ServiceError> {
    GameContainer::reload_game(game_id, request_context).await
}
