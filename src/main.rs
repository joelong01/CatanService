/**
 *  main entry point for the application.  The goal here is to set up the Web Server.
 */
mod cosmos_db;
mod games_service;
mod macros;
mod middleware;
mod shared;
mod test;
mod user_service;

// dependencies...

use actix_web::{web, HttpResponse, HttpServer, Scope};

use crate::games_service::{game_container::game_container, lobby::lobby_handlers};
use games_service::game_handlers;
use lazy_static::lazy_static;
use log::error;
use middleware::authn_mw::AuthenticationMiddlewareFactory;
use middleware::environment_mw::{
    EnvironmentMiddleWareFactory, ServiceEnvironmentContext, CATAN_ENV,
};
use once_cell::sync::OnceCell;
use openssl::ssl::{SslAcceptor, SslFiletype, SslMethod};
use std::env;
use std::sync::atomic::{AtomicBool, Ordering};
use user_service::users;

/**
 *  Code to pick a port in a threadsafe way -- either specified in an environment variable named COSMOS_RUST_SAMPLE_PORT
 *  or 8080
 */
static PORT: OnceCell<String> = OnceCell::new();

#[allow(unused_macros)]
#[macro_export]
macro_rules! safe_set_port {
    () => {{
        let port: String;
        match PORT.get() {
            Some(val) => {
                port = val.to_string();
            }
            None => {
                match env::var("CATAN_APP_PORT") {
                    Ok(val) => port = val.to_string(),
                    Err(_e) => port = "8080".to_string(),
                }
                println!("setting port to: {}", port);
                match PORT.set(port.clone()) {
                    Ok(_) => {}
                    Err(e) => {
                        println!("error setting port: {:?}", e.to_string());
                    }
                }
            }
        };
        port
    }};
}
/**
 *  main:  entry point that sets up the web service
 */
#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Access CATAN_SECRETS to force initialization and potentially panic.
    print!("env_logger set with {:#?}\n", CATAN_ENV.rust_log);
    print!("ssl key file {:#?}\n", CATAN_ENV.ssl_key_location);
    print!("ssl cert file {:#?}\n", CATAN_ENV.ssl_cert_location);
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let port: String = safe_set_port!();

    //
    //  SSL support
    let mut builder = SslAcceptor::mozilla_intermediate(SslMethod::tls()).unwrap();
    builder
        .set_private_key_file(CATAN_ENV.ssl_key_location.to_owned(), SslFiletype::PEM)
        .unwrap();
    builder
        .set_certificate_chain_file(CATAN_ENV.ssl_cert_location.to_owned())
        .unwrap();

    //
    // set up the HttpServer - pass in the broker service as part of App data
    // we use the create_app! macro so that we always create the same shape of app in our tests
    HttpServer::new(move || create_app!())
        .bind_openssl(format!("0.0.0.0:{}", port), builder)?
        .run()
        .await
}

/**
 * this is the simplest possible GET handler that can be run from a browser to test connectivity
 */
async fn get_version() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/plain")
        .body("version 1.0")
}

/**
 * Creates a set of unauthenticated services under the "/api/v1" path.
 * These endpoints are accessible without any user authentication and are mainly used for:
 *
 * - Version Information:
 *   - Retrieves the version information of the application.
 *   - URL: `https://localhost:8080/api/v1/version`
 *   - Method: `GET`
 *
 * - User Registration:
 *   - Registers a new user with the provided information.
 *   - URL: `https://localhost:8080/api/v1/users/register`
 *   - Method: `POST`
 *
 * - User Login:
 *   - Authenticates a user and returns a token or session information.
 *   - URL: `https://localhost:8080/api/v1/users/login`
 *   - Method: `POST`
 *
 * - Test Setup:
 *   - A special endpoint used only for testing purposes to set up test data.
 *   - URL: `https://localhost:8080/api/v1/test/setup`
 *   - Method: `POST`
 */
fn create_unauthenticated_service() -> Scope {
    web::scope("/api").service(
        web::scope("/v1")
            .route("/version", web::get().to(get_version))
            .route("/users/register", web::post().to(users::register))
            .route("/users/login", web::post().to(users::login))
            .route("/test/setup", web::post().to(users::setup)), /* TEST ONLY */
    )
}

/**
 * Creates a set of user-related services under the "/users" path.
 * These endpoints allow for user management and are typically restricted to authenticated users:
 *
 * - List Users:
 *   - Retrieves a list of all users in the system.
 *   - URL: `https://localhost:8080/auth/api/v1/users`
 *   - Method: `GET`
 *
 * - Delete User:
 *   - Deletes a user with the given ID.
 *   - URL: `https://localhost:8080/auth/api/v1/users/{id}` (replace `{id}` with the user's ID)
 *   - Method: `DELETE`
 *
 * - Find User by ID:
 *   - Retrieves details of a specific user by their ID.
 *   - URL: `https://localhost:8080/auth/api/v1/users/{id}` (replace `{id}` with the user's ID)
 *   - Method: `GET`
 */
fn user_service() -> Scope {
    web::scope("/users")
        .route("", web::get().to(users::list_users))
        .route("/{id}", web::delete().to(users::delete))
        .route("/{id}", web::get().to(users::find_user_by_id))
}

/**
 * Creates a set of lobby-related services under the "/lobby" path.
 * These endpoints allow for handling lobby operations within the game, such as inviting and joining games.
 * They are typically restricted to authenticated users:
 *
 * - Get Lobby:
 *   - Retrieves information about the current lobby.
 *   - URL: `https://localhost:8080/auth/api/v1/lobby`
 *   - Method: `GET`
 *
 * - Invite to Lobby:
 *   - Sends an invite to another user to join the current lobby.
 *   - URL: `https://localhost:8080/auth/api/v1/lobby/invite`
 *   - Method: `POST`
 *
 * - Join Game:
 *   - Allows a user to join a game via the lobby.
 *   - URL: `https://localhost:8080/auth/api/v1/lobby/joingame`
 *   - Method: `POST`
 */
fn lobby_service() -> Scope {
    web::scope("/lobby")
        .route("", web::get().to(lobby_handlers::get_lobby))
        .route("/invite", web::post().to(lobby_handlers::post_invite))
        .route("/acceptinvite", web::post().to(lobby_handlers::respond_to_invite))
}

/**
 * Creates a set of game-related services under the "/games" path.
 * These endpoints enable various game operations, such as fetching supported games, creating a new game, and shuffling an existing game.
 * They are typically restricted to authenticated users:
 *
 * - Supported Games:
 *   - Retrieves information about the supported game types.
 *   - URL: `https://localhost:8080/auth/api/v1/games/`
 *   - Method: `GET`
 *
 * - New Game:
 *   - Creates a new game of the specified type.
 *   - URL: `https://localhost:8080/auth/api/v1/games/{game_type}`
 *   - Method: `POST`
 *
 * - Shuffle Game:
 *   - Initiates the shuffling of the specified game.
 *   - URL: `https://localhost:8080/auth/api/v1/games/shuffle/{game_id}`
 *   - Method: `POST`
 */
fn game_service() -> Scope {
    web::scope("/games")
        .route("/", web::get().to(game_handlers::supported_games))
        .route("/{game_type}", web::post().to(game_handlers::new_game))
        .route(
            "/shuffle/{game_id}",
            web::post().to(game_handlers::shuffle_game),
        )
        .route("/join/{game_id}", web::post().to(game_handlers::join_game))
}

fn longpoll_service() -> Scope {
    web::scope("/longpoll").route("", web::get().to(game_container::long_poll_handler))
}

fn profile_service() -> Scope {
    web::scope("profile").route("", web::get().to(users::get_profile))
}

lazy_static! {
    static ref LOGGER_INIT: AtomicBool = AtomicBool::new(false);
    static ref LOGGER_INIT_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::new(());
}

pub async fn init_env_logger() {
    if LOGGER_INIT.load(Ordering::Relaxed) {
        full_info!("env logger already created");
        return;
    }
    let _lock_guard = LOGGER_INIT_LOCK.lock().await;
    if LOGGER_INIT.load(Ordering::Relaxed) {
        full_info!("env logger already created");
        return;
    }

    match env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .filter(
            Some("catan_service::cosmos_db::cosmosdb"),
            log::LevelFilter::Off,
        )
        .try_init()
    {
        Ok(()) => {
            full_info!(
                "logger initialized - NOTE: catan_service::cosmos_db::cosmosdb is always OFF!"
            );
        }
        Err(e) => error!("logger failt to init -- already inited?: {:#?}", e),
    }
    LOGGER_INIT.store(true, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use crate::{
        create_test_service,
        games_service::game_container::game_messages::GameHeaders,
        init_env_logger, setup_test,
        shared::models::{ClientUser, ServiceResponse, UserProfile},
    };
    use actix_web::{http::header, test};

    use log::trace;
    use reqwest::StatusCode;
    use serde_json::json;

    #[actix_rt::test]
    async fn test_version_and_log_intialized() {
        init_env_logger().await;
        init_env_logger().await;
        let mut app = create_test_service!();
        let req = test::TestRequest::get().uri("/api/v1/version").to_request();

        let resp = test::call_service(&mut app, req).await;
        assert_eq!(resp.status(), 200);

        // You can also assert the response body if you want.
        let body = test::read_body(resp).await;
        assert_eq!(body, "version 1.0");
    }

    #[actix_rt::test]
    async fn create_user_login_check_profile() {
        let mut app = create_test_service!();
        setup_test!(&app);

        const USER_1_PASSWORD: &'static str = "password";

        // 1. Register the user
        let mut user1_profile = UserProfile {
            email: "testuser@example.com".into(),
            first_name: "Test".into(),
            last_name: "User".into(),
            display_name: "TestUser".into(),
            picture_url: "https://example.com/photo.jpg".into(),
            foreground_color: "#000000".into(),
            background_color: "#FFFFFF".into(),
            text_color: "#000000".into(),
            games_played: Some(10),
            games_won: Some(1),
        };
        let req = test::TestRequest::post()
            .uri("/api/v1/users/register")
            .append_header((header::CONTENT_TYPE, "application/json"))
            .append_header((GameHeaders::IS_TEST, "true"))
            .append_header((GameHeaders::PASSWORD, USER_1_PASSWORD))
            .set_json(&user1_profile)
            .to_request();

        let response = test::call_service(&mut app, req).await;
        let status = response.status();
        let body = test::read_body(response).await;
        if status != 200 {
            trace!("user_1 already registered");
            assert_eq!(status, 409);
            let resp: ServiceResponse =
                serde_json::from_slice(&body).expect("failed to deserialize into ServiceResponse");
            assert_eq!(resp.status, 409);
            assert_eq!(resp.body, "");
        } else {
            // Deserialize the body into a ClientUser object
            let client_user: ClientUser =
                serde_json::from_slice(&body).expect("Failed to deserialize response body");

            let pretty_json =
                serde_json::to_string_pretty(&client_user).expect("Failed to pretty-print JSON");

            // Check if the pretty-printed JSON contains any underscores
            assert!(
                !pretty_json.contains('_'),
                "JSON contains an underscore character"
            );

            trace!("registered client_user: {:#?}", pretty_json);
        }

        // 2. Login the user
        let login_payload = json!({
            "username": user1_profile.email,
            "password": USER_1_PASSWORD
        });
        let req = test::TestRequest::post()
            .uri("/api/v1/users/login")
            .append_header((GameHeaders::IS_TEST, "true"))
            .set_json(&login_payload)
            .to_request();

        let resp = test::call_service(&mut app, req).await;
        assert_eq!(resp.status(), 200);

        let body = test::read_body(resp).await;
        let service_response: ServiceResponse =
            serde_json::from_slice(&body).expect("failed to deserialize into ServiceResponse");

        // Extract auth token from response
        let auth_token = service_response.body;
        assert!(auth_token.len() > 10, "auth token appears invalid");

        // 4. Get profile
        let req = test::TestRequest::get()
            .uri("/auth/api/v1/profile")
            .append_header((header::CONTENT_TYPE, "application/json"))
            .append_header((GameHeaders::IS_TEST, "true"))
            .append_header(("Authorization", auth_token))
            .to_request();

        let resp = test::call_service(&mut app, req).await;
        assert_eq!(resp.status(), 200);
        let body = test::read_body(resp).await;
        let profile_from_service: UserProfile =
            serde_json::from_slice(&body).expect("error deserializing profile");
        user1_profile.games_played = Some(0);
        user1_profile.games_won = Some(0); // service sets this when regisering.
        assert!(
            profile_from_service.is_equal_byval(&user1_profile),
            "profile returned by service different than the one sent in"
        );
    }
    #[actix_rt::test]
    async fn test_setup_no_test_header() {
        let mut app = create_test_service!();

        let request = test::TestRequest::post()
            .uri("/api/v1/test/setup")
            .to_request();

        let response = test::call_service(&mut app, request).await;

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[actix_rt::test]
    pub async fn find_or_create_test_db() {
        let app = create_test_service!();
        setup_test!(&app);
    }
}
