mod azure_setup;
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

use actix_web::{web, HttpResponse, HttpServer, Scope};

use games_service::actions::action_handlers;
use games_service::long_poller::long_poller_handler::long_poll_handler;
use shared::models::ServiceResponse;

use std::env;
use std::net::ToSocketAddrs;

use crate::azure_setup::azure_wrapper::verify_or_create_account;
use crate::azure_setup::azure_wrapper::verify_or_create_collection;
use crate::azure_setup::azure_wrapper::verify_or_create_database;
use crate::games_service::lobby::lobby_handlers;
use games_service::game_handlers;
use lazy_static::lazy_static;
use log::{error, LevelFilter};
use middleware::authn_mw::AuthenticationMiddlewareFactory;
use middleware::environment_mw::CATAN_ENV;
use openssl::ssl::{SslAcceptor, SslFiletype, SslMethod};
use std::sync::atomic::{AtomicBool, Ordering};
use user_service::user_handlers;

pub use log::info;
pub use log::trace;

fn get_host_ip_and_port() -> (String, String) {
    let host_name = std::env::var("HOST_NAME").expect("HOST_NAME must be set");
    let parts: Vec<&str> = host_name.split(':').collect();

    let hostname = parts[0];
    let port: u16 = parts.get(1).unwrap_or(&"8080").parse().unwrap_or(8080);

    // Resolve the server name to an IP address
    let mut addrs = format!("{}:{}", hostname, port)
        .to_socket_addrs()
        .expect("Failed to resolve domain name");

    let ip_address = addrs
        .next()
        .expect("No IP address found for the domain name")
        .ip();

    (ip_address.to_string(), port.to_string())
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
    let args: Vec<String> = env::args().collect();

    if args.len() > 1 && args[1] == "--setup" {
        setup_cosmos().expect("Setup failed and the app cannot continue.");
    }

    let (ip_address, port) = get_host_ip_and_port();

    println!("Binding to IP: {}:{}", ip_address, port);

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
        .bind_openssl(format!("{}:{}", ip_address, port), builder)?
        .run()
        .await
}

 fn setup_cosmos() -> Result<(), ServiceResponse> {
    verify_or_create_account(
        &CATAN_ENV.resource_group,
        &CATAN_ENV.cosmos_account,
        &CATAN_ENV.azure_location,
    )?;

    verify_or_create_database(
        &CATAN_ENV.cosmos_account,
        &CATAN_ENV.cosmos_database_name,
        &CATAN_ENV.resource_group,
    )?;
    let test_db_name = CATAN_ENV.cosmos_database_name.to_string() + "-test";
    verify_or_create_database(
        &CATAN_ENV.cosmos_account,
        &test_db_name,
        &CATAN_ENV.resource_group,
    )?;

    for collection in &CATAN_ENV.cosmos_collections {
        verify_or_create_collection(
            &CATAN_ENV.cosmos_account,
            &CATAN_ENV.cosmos_database_name,
            &collection,
            &CATAN_ENV.resource_group,
        )?;

        let test_name = collection.clone() + "-test";
        verify_or_create_collection(
            &CATAN_ENV.cosmos_account,
            &test_db_name,
            &test_name,
            &CATAN_ENV.resource_group,
        )?;
    }
    Ok(())
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
 *   - URL: `https://localhost:8080/api/v1/test/verify_service`
 *   - Method: `POST`
 */
fn create_unauthenticated_service() -> Scope {
    web::scope("/api").service(
        web::scope("/v1")
            .route("/version", web::get().to(get_version))
            .route(
                "/users/register",
                web::post().to(user_handlers::register_handler),
            )
            .route("/users/login", web::post().to(user_handlers::login_handler))
            .route("/test/verify_service", web::post().to(user_handlers::verify_handler)) /* TEST ONLY */
            .route(
                "/users/validate_email/{token}",
                web::get().to(user_handlers::validate_email),
            ),
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
        .route("", web::get().to(user_handlers::list_users_handler))
        .route("/{id}", web::delete().to(user_handlers::delete_handler))
        .route(
            "/{id}",
            web::get().to(user_handlers::find_user_by_id_handler),
        )
        .route(
            "/phone/validate/{code}",
            web::post().to(user_handlers::validate_phone_handler),
        )
        .route(
            "/phone/send_code",
            web::post().to(user_handlers::send_phone_code_handler),
        )
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
        .route(
            "/acceptinvite",
            web::post().to(lobby_handlers::respond_to_invite),
        )
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
}

fn action_service() -> Scope {
    web::scope("/action")
        .route("/start/{game_id}", web::post().to(action_handlers::next))
        .route(
            "/actions/{game_id}",
            web::get().to(action_handlers::valid_actions),
        )
        .route("/next/{game_id}", web::post().to(action_handlers::next))
}

fn longpoll_service() -> Scope {
    web::scope("/longpoll/{index}").route("", web::get().to(long_poll_handler))
}

fn profile_service() -> Scope {
    web::scope("profile").route("", web::get().to(user_handlers::get_profile_handler))
}

lazy_static! {
    static ref LOGGER_INIT: AtomicBool = AtomicBool::new(false);
    static ref LOGGER_INIT_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::new(());
}

pub async fn init_env_logger(min_level: LevelFilter, cosmos_log_level: LevelFilter) {
    if LOGGER_INIT.load(Ordering::Relaxed) {
        full_info!("env logger already created");
        return;
    }
    let _lock_guard = LOGGER_INIT_LOCK.lock().await;
    if LOGGER_INIT.load(Ordering::Relaxed) {
        full_info!("env logger already created");
        return;
    }

    // Start by setting the global filter level to `min_level`
    let mut builder =
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"));
    builder.filter(None, min_level);

    // Then, set the module-specific filter level
    builder.filter(Some("catan_service::cosmos_db::cosmosdb"), cosmos_log_level);

    match builder.try_init() {
        Ok(()) => {
            full_info!(
                "logger initialized [min_level: {:#?}] [cosmos_min_level: {:#?}]",
                min_level,
                cosmos_log_level
            );
        }
        Err(e) => error!("logger failed to init -- already inited?: {:#?}", e),
    }
    LOGGER_INIT.store(true, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use crate::{
        create_test_service,
        games_service::game_container::game_messages::GameHeader,
        init_env_logger,
        middleware::environment_mw::{RequestContext, TestContext, CATAN_ENV},
        setup_test,
        shared::models::{ClientUser, ServiceResponse, UserProfile},
        user_service::users::{login, register, verify_cosmosdb}, setup_cosmos,
    };

    use actix_web::{http::header, test};

    use log::trace;
    use reqwest::StatusCode;

    #[actix_rt::test]
    async fn test_version_and_log_intialized() {
        init_env_logger(log::LevelFilter::Trace, log::LevelFilter::Error).await;
        init_env_logger(log::LevelFilter::Trace, log::LevelFilter::Error).await;
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
        setup_test!(&app, false);

        const USER_1_PASSWORD: &'static str = "password";

        // 1. Register the user
        let mut user1_profile = UserProfile {
            email: "testuser@example.com".into(),
            first_name: "Test".into(),
            last_name: "User".into(),
            display_name: "TestUser".into(),
            phone_number: crate::middleware::environment_mw::CATAN_ENV
                .test_phone_number
                .to_owned(),
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
            .append_header((GameHeader::TEST, TestContext::as_json(false)))
            .append_header((GameHeader::PASSWORD, USER_1_PASSWORD))
            .set_json(&user1_profile)
            .to_request();

        let response = test::call_service(&mut app, req).await;
        let status = response.status();
        let body = test::read_body(response).await;
        if !status.is_success() {
            trace!("user_1 already registered");
            assert_eq!(status, 409);
            let resp: ServiceResponse =
                serde_json::from_slice(&body).expect("failed to deserialize into ServiceResponse");
            assert_eq!(resp.status, 409);
        } else {
            //  we get back a service response with a client user in the body

            let client_user: ClientUser =
                ServiceResponse::to_client_user(std::str::from_utf8(&body).unwrap())
                    .expect("Service Response should deserialize")
                    .1;

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

        let req = test::TestRequest::post()
            .uri("/api/v1/users/login")
            .append_header((GameHeader::TEST, TestContext::as_json(false)))
            .append_header((GameHeader::PASSWORD, USER_1_PASSWORD))
            .append_header((GameHeader::EMAIL, user1_profile.email.clone()))
            .to_request();

        let resp = test::call_service(&mut app, req).await;
        assert!(resp.status().is_success());

        let body = test::read_body(resp).await;
        let auth_token = ServiceResponse::json_to_token(std::str::from_utf8(&body).unwrap())
            .expect("should be jwt")
            .1;

        assert!(auth_token.len() > 10, "auth token appears invalid");

        // 4. Get profile
        let req = test::TestRequest::get()
            .uri("/auth/api/v1/profile")
            .append_header((header::CONTENT_TYPE, "application/json"))
            .append_header((GameHeader::TEST, TestContext::as_json(false)))
            .append_header(("Authorization", auth_token))
            .to_request();

        let resp = test::call_service(&mut app, req).await;
        assert_eq!(resp.status(), 200);
        let body = test::read_body(resp).await;
        //
        //  we get a service response where the body is a ClientUser
        let profile_from_service =
            ServiceResponse::to_client_user(std::str::from_utf8(&body).unwrap())
                .expect("Service Response should deserialize")
                .1;

        user1_profile.games_played = Some(0);
        user1_profile.games_won = Some(0); // service sets this when regisering.
        assert!(
            profile_from_service
                .user_profile
                .is_equal_byval(&user1_profile),
            "profile returned by service different than the one sent in"
        );
    }
    #[actix_rt::test]
    async fn test_setup_no_test_header() {
        let mut app = create_test_service!();

        let request = test::TestRequest::post()
            .uri("/api/v1/test/verify_service")
            .to_request();

        let response = test::call_service(&mut app, request).await;

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[actix_rt::test]
    pub async fn find_or_create_test_db() {
        let app = create_test_service!();
        setup_test!(&app, false);
    }

    #[tokio::test]

    async fn test_validate_phone() {
        init_env_logger(log::LevelFilter::Info, log::LevelFilter::Error).await;
        let mut app = create_test_service!();
        setup_test!(&app, false);

        let request_context = RequestContext::test_default(false);
        let mut profile = UserProfile::new_test_user();
        profile.phone_number = CATAN_ENV.test_phone_number.clone();
        // setup
        let _response = verify_cosmosdb(&request_context).await;

        let sr = register("password", &profile, &request_context)
            .await
            .expect("this should work");
        let _client_user = sr.get_client_user().expect("This should be a client user");

        let response = login(&profile.email, "password", &request_context)
            .await
            .expect("login should succeed");

        let auth_token = response.get_token().expect("an auth token!");

        let req = test::TestRequest::post()
            .uri("/auth/api/v1/users/phone/send_code")
            .append_header((header::CONTENT_TYPE, "application/json"))
            .append_header((GameHeader::TEST, TestContext::as_json(false)))
            .append_header(("Authorization", auth_token.clone()))
            .to_request();

        let response = test::call_service(&mut app, req).await;
        assert_eq!(response.status(), 200);

        let body = test::read_body(response).await;
        let phone_code = ServiceResponse::json_to_token(std::str::from_utf8(&body).unwrap())
            .expect("should be jwt")
            .1;

        let req = test::TestRequest::post()
            .uri(&format!("/auth/api/v1/users/phone/validate/{}", phone_code))
            .append_header((header::CONTENT_TYPE, "application/json"))
            .append_header((GameHeader::TEST, TestContext::as_json(false)))
            .append_header(("Authorization", auth_token))
            .to_request();
        let response = test::call_service(&mut app, req).await;
        assert_eq!(response.status(), 200);
    }
    #[actix_rt::test]
    async fn test_setup() {
        init_env_logger(log::LevelFilter::Trace, log::LevelFilter::Trace).await;
        setup_cosmos().expect("can't continue if setup fails!");
    }

}
