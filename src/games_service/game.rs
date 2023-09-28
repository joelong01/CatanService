use crate::{
    games_service::{
        game_container::game_messages::{CatanMessage, GameCreatedData},
        long_poller::long_poller::LongPoller,
    },
    middleware::request_context_mw::RequestContext,
    shared::shared_models::{UserProfile, GameError, ResponseType, ServiceResponse}, full_info,
};

use reqwest::StatusCode;

use crate::games_service::shared::game_enums::CatanGames;

use super::{
    catan_games::{games::regular::regular_game::RegularGame, traits::game_trait::GameTrait},
    game_container::game_container::GameContainer,
};

///
/// check the state to make sure the request is valid
/// randomize the board and the harbors
/// post the response to websocket
pub async fn shuffle_game(game_id: &str, request_context: &RequestContext) -> Result<ServiceResponse, ServiceResponse> {
    
    if request_context.is_test() {
        full_info!("test shuffle");
    }
    
    let (game, _) = GameContainer::current_game(&game_id.to_owned()).await?;

    let mut new_game = game.clone();
    new_game.shuffle_count = new_game.shuffle_count + 1;
    new_game.shuffle();
    let result = GameContainer::push_game(&game_id.to_owned(), &new_game).await;
    match result {
        Ok(_) => Ok(ServiceResponse::new(
            "shuffled",
            StatusCode::OK,
            ResponseType::Game(new_game),
            GameError::NoError(String::default()),
        )),
        Err(e) => {
            let err_message = format!("GameContainer::push_game error: {:#?}", e);
            return Err(ServiceResponse::new(
                "Error Hashing Password",
                StatusCode::INTERNAL_SERVER_ERROR,
                ResponseType::ErrorInfo(err_message.to_owned()),
                GameError::HttpError(StatusCode::INTERNAL_SERVER_ERROR),
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
    game_type: CatanGames,
    user_id: &str,
    is_test: bool,
    test_game: Option<RegularGame>,
    request_context: &RequestContext,
) -> Result<ServiceResponse, ServiceResponse> {
    if game_type != CatanGames::Regular {
        return Err(ServiceResponse::new(
            &format!("Game not supported: {:#?}", game_type),
            StatusCode::BAD_REQUEST,
            ResponseType::NoData,
            GameError::MissingData(String::default()),
        ));
    }
    let user = request_context
        .database
        .find_user_by_id(user_id)
        .await?;

    //
    //  "if it is a test game and the game has been passed in, use it.  otherwise create a new game and shuffle"
    let game = if is_test {
        match test_game {
            Some(g) => g.clone(),
            None => {
                let mut game = RegularGame::new(&UserProfile::from_persist_user(&user));
                game.shuffle();
                game
            }
        }
    } else {
        let mut game = RegularGame::new(&UserProfile::from_persist_user(&user));
        game.shuffle();
        game
    };

    //
    //  the sequence is
    //  1. create_and_add_container
    //  2. push_game
    //  3. add_player
    //  4. send notification
    if GameContainer::create_and_add_container(&game.game_id, &game)
        .await
        .is_err()
    {
        return Err(ServiceResponse::new(
            "",
            reqwest::StatusCode::NOT_FOUND,
            ResponseType::NoData,
            GameError::BadId(game.game_id.to_owned()),
        ));
    }

    //
    //  send a message to the user that the game was created

    let _ = LongPoller::send_message(
        vec![user_id.to_string()],
        &CatanMessage::GameCreated(GameCreatedData {
            user_id: user_id.to_string(),
            game_id: game.game_id.to_string(),
        }),
    )
    .await;

    //
    //  return the game - perhaps this shouldn't be done to force parity between the caller and the other clients
    //  - make them all get the game from the long poller.  as it is the client will set the context - forcing an update
    //  and then get the update from the long_polller, which will do the same thing.  we might just ignore the return
    //  value on the client, in which case we are wasting bytes on the wire.
    Ok(ServiceResponse::new(
        "shuffled",
        StatusCode::OK,
        ResponseType::Game(game),
        GameError::NoError(String::default()),
    ))
}

pub async fn supported_games() -> Result<ServiceResponse, ServiceResponse> {
    Ok(ServiceResponse::new(
        "shuffled",
        StatusCode::OK,
        ResponseType::SupportedGames(vec![CatanGames::Regular]),
        GameError::NoError(String::default()),
    ))
}
