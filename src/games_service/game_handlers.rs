use crate::{
    full_info,
    games_service::{
        game_container::game_messages::{CatanMessage, GameCreatedData},
        long_poller::long_poller::LongPoller,
    },
    middleware::environment_mw::ServiceEnvironmentContext,
    shared::models::{ClientUser, ServiceResponse},
    trace_function, trace_thread_info,
    user_service::users::{create_http_response, internal_find_user},
};
use actix_web::{
    web::{self, Data, Path},
    HttpRequest, HttpResponse, Responder,
};
use azure_core::StatusCode;

use scopeguard::defer;

use crate::games_service::shared::game_enums::{CatanGames, SupportedGames};

use super::{
    catan_games::{
        games::regular::regular_game::RegularGame,
        traits::{game_state_machine_trait::StateMachineTrait, game_trait::CatanGame},
    },
    game_container::{game_container::GameContainer, game_messages::GameHeader},
    shared::game_enums::GameState,
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
            let result = GameContainer::push_game(&game_id.to_owned(), &new_game).await;
            match result {
                Ok(_) => HttpResponse::Ok()
                    .content_type("application/json")
                    .json(game),
                Err(e) => create_http_response(
                    StatusCode::InternalServerError,
                    &format!("GameContainer::push_game error: {:#?}", e),
                    "",
                ),
            }
        }
        Err(_e) => {
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
    test_game: Option<web::Json<RegularGame>>,
) -> impl Responder {
    trace_function!("new_game", "");
    let game_type = game_type.into_inner();
    let user_id = req
        .headers()
        .get(GameHeader::USER_ID)
        .unwrap()
        .to_str()
        .unwrap();

    if game_type != CatanGames::Regular {
        return create_http_response(
            StatusCode::BadRequest,
            &format!("Game not supported: {:#?}", game_type),
            "",
        );
    }

    let is_test = req.headers().contains_key(GameHeader::IS_TEST);
    let user_result = internal_find_user("id", user_id, is_test, &data).await;

    let user = match user_result {
        Ok(u) => u,
        Err(_) => {
            return create_http_response(
                StatusCode::NotFound,
                &format!("invalid user id: {}", user_id),
                "",
            );
        }
    };
    //
    //  "if it is a test game and the game has been passed in, use it.  otherwise create a new game and shuffle"
    let game = if is_test {
        match test_game {
            Some(g) => g.clone(),
            None => {
                let mut game = RegularGame::new(&ClientUser::from_persist_user(user));
                game.shuffle();
                game
            }
        }
    } else {
        let mut game = RegularGame::new(&ClientUser::from_persist_user(user));
        game.shuffle();
        game
    };

    //
    //  the sequence is
    //  1. create_and_add_container
    //  2. push_game
    //  3. add_player
    //  4. send notification
    match GameContainer::create_and_add_container(&game.id, &game).await {
        Err(e) => {
            full_info!("insert_container returned an error.  {:#?}!", e);
            return create_http_response(StatusCode::NotFound, &format!("{:?}", e), "");
        }
        Ok(_) => {}
    }

    //
    //  send a message to the user that the game was created
    trace_thread_info!("new_game", "Lobby::game_created");
    let _ = LongPoller::send_message(
        vec![user_id.to_string()],
        &CatanMessage::GameCreated(GameCreatedData {
            user_id: user_id.to_string(),
            game_id: game.id.to_string(),
        }),
    )
    .await;

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

pub async fn start_game(game_id_path: web::Path<String>, _req: HttpRequest) -> impl Responder {
    let game_id: &str = &game_id_path;

    let game = match GameContainer::current(&game_id.to_owned()).await {
        Ok(g) => g,
        Err(e) => {
            return create_http_response(
                StatusCode::NotFound,
                &format!("invalid game_id: {}.  error: {:?}", game_id, e),
                "",
            )
        }
    };
   // let next_state = game.state_data.state()
    let mut game_clone = game.clone();
    game_clone.set_current_state(GameState::ChoosingBoard);
    let _ = GameContainer::push_game(game_id, &game_clone).await;
    HttpResponse::Ok().into()
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
                StatusCode::NotFound,
                &format!("invalid game_id: {}.  error: {:?}", game_id, e),
                "",
            )
        }
    };
    HttpResponse::Ok()
        .content_type("application/json")
        .json(game.state_data.actions())
}
