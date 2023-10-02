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

use cosmos_db::database_abstractions::COLLECTION_NAME_VALUES;
use games_service::actions::action_handlers;
use games_service::long_poller::long_poller_handler::long_poll_handler;
use shared::shared_models::ServiceError;

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
use middleware::service_config::SERVICE_CONFIG;
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
    // print!("env_logger set with {:#?}\n", SERVICE_CONFIG.rust_log);
    // print!("ssl key file {:#?}\n", SERVICE_CONFIG.ssl_key_location);
    // print!("ssl cert file {:#?}\n", SERVICE_CONFIG.ssl_cert_location);
    // env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    init_env_logger(log::LevelFilter::Info, log::LevelFilter::Error).await;
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
        .set_private_key_file(SERVICE_CONFIG.ssl_key_location.to_owned(), SslFiletype::PEM)
        .unwrap();
    builder
        .set_certificate_chain_file(SERVICE_CONFIG.ssl_cert_location.to_owned())
        .unwrap();

    //
    // set up the HttpServer - pass in the broker service as part of App data
    // we use the create_app! macro so that we always create the same shape of app in our tests
    HttpServer::new(move || create_service!())
        .bind_openssl(format!("{}:{}", ip_address, port), builder)?
        .run()
        .await
}

fn setup_cosmos() -> Result<(), ServiceError> {
    verify_or_create_account(
        &SERVICE_CONFIG.resource_group,
        &SERVICE_CONFIG.cosmos_account,
        &SERVICE_CONFIG.azure_location,
    )?;

    verify_or_create_database(
        &SERVICE_CONFIG.cosmos_account,
        &SERVICE_CONFIG.cosmos_database_name,
        &SERVICE_CONFIG.resource_group,
    )?;
    let test_db_name = SERVICE_CONFIG.cosmos_database_name.to_string() + "-test";
    verify_or_create_database(
        &SERVICE_CONFIG.cosmos_account,
        &test_db_name,
        &SERVICE_CONFIG.resource_group,
    )?;

    for collection in COLLECTION_NAME_VALUES.iter() {
        verify_or_create_collection(
            &SERVICE_CONFIG.cosmos_account,
            &SERVICE_CONFIG.cosmos_database_name,
            collection.value,
            &SERVICE_CONFIG.resource_group,
        )?;

        let test_name = format!("{}-test", collection.value);
        verify_or_create_collection(
            &SERVICE_CONFIG.cosmos_account,
            &test_db_name,
            &test_name,
            &SERVICE_CONFIG.resource_group,
        )?;
    }
    Ok(())
}

/**
 * this is the simplest possible GET handler that can be run from a browser to test connectivity
 */
async fn get_version() -> HttpResponse {
    HttpResponse::Ok().body("version 1.0")
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
 *   - URL: `https://localhost:8080/api/v1/test/verify-service`
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
            .route(
                "/test/verify-service",
                web::post().to(user_handlers::verify_handler),
            ) /* TEST ONLY */
            .route(
                "/users/validate-email/{token}",
                web::get().to(user_handlers::validate_email_handler),
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
        .route(
            "/local",
            web::post().to(user_handlers::create_local_user_handler),
        )
        .route(
            "/local/{id}",
            web::get().to(user_handlers::get_local_users_handler),
        )
        .route(
            "/local/{id}",
            web::delete().to(user_handlers::delete_local_user_handler),
        )
        .route(
            "/local",
            web::put().to(user_handlers::update_local_user_handler),
        )
        .route("/{id}", web::delete().to(user_handlers::delete_handler))
        .route(
            "/{id}",
            web::get().to(user_handlers::find_user_by_id_handler),
        )
        .route(
            "/{id}",
            web::put().to(user_handlers::update_profile_handler),
        )
        .route(
            "/phone/validate/{code}",
            web::post().to(user_handlers::validate_phone_handler),
        )
        .route(
            "/phone/send-code",
            web::post().to(user_handlers::send_phone_code_handler),
        )
        .route(
            "/email/send-validation-email",
            web::post().to(user_handlers::send_validation_email_handler),
        )
        .route(
            "/register-test-user",
            web::post().to(user_handlers::register_test_user_handler),
        )
        .route(
            "/rotate-login-keys",
            web::post().to(user_handlers::rotate_login_keys_handler),
        )
}
// fn local_user_service() -> Scope {

// }
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
        .route(
            "/add-local-user/{local_user_id}",
            web::post().to(lobby_handlers::add_local_user_handler),
        )
        .route("/join", web::post().to(lobby_handlers::join_lobby_handler))
        .route(
            "/leave",
            web::post().to(lobby_handlers::leave_lobby_handler),
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
        .route("/", web::get().to(game_handlers::supported_games_handler))
        .route(
            "/{game_type}",
            web::post().to(game_handlers::new_game_handler),
        )
        .route(
            "/shuffle/{game_id}",
            web::post().to(game_handlers::shuffle_game),
        )
        .route(
            "/reload/{game_id}",
            web::post().to(game_handlers::reload_game_handler),
        )
}

fn action_service() -> Scope {
    web::scope("/action")
        .route(
            "/start/{game_id}",
            web::post().to(action_handlers::next_handler),
        )
        .route(
            "/actions/{game_id}",
            web::get().to(action_handlers::valid_actions),
        )
        .route(
            "/next/{game_id}",
            web::post().to(action_handlers::next_handler),
        )
}

fn longpoll_service() -> Scope {
    web::scope("/longpoll").route("", web::get().to(long_poll_handler))
}

fn profile_service() -> Scope {
    web::scope("profile").route(
        "/{email}",
        web::get().to(user_handlers::get_profile_handler),
    )
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
    builder.filter(Some("actix_server::worker"), LevelFilter::Error);

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
        create_service, create_test_service,
        games_service::game_container::game_messages::GameHeader,
        init_env_logger,
        middleware::{request_context_mw::TestContext, service_config::SERVICE_CONFIG},
        setup_cosmos, setup_test,
        test::{test_proxy::TestProxy, test_helpers::test::TestHelpers},
    };

    use actix_web::test;
    use reqwest::StatusCode;

    #[tokio::test]
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

    #[tokio::test]
    async fn create_user_login_check_profile() {
        init_env_logger(log::LevelFilter::Info, log::LevelFilter::Error).await;

        let app = create_test_service!();
        setup_test!(&app, false);
        let use_cosmos = true;
        let mut proxy = TestProxy::new(&app, Some(TestContext::new(use_cosmos, None, None)));

        let test_users = TestHelpers::register_test_users(&mut proxy, None).await;
        let auth_token = proxy
            .login(
                &test_users[0].clone().pii.expect("pii should exist").email,
                "password",
            )
            .await
            .expect("success");

        proxy.set_auth_token(Some(auth_token));
        let profile = proxy.get_profile("Self").await.expect("success");
        let first = test_users
            .first()
            .and_then(|user| user.pii.as_ref())
            .unwrap();

        assert_eq!(&profile.pii.as_ref().unwrap().email, first.email.as_str());
    }
    #[tokio::test]
    async fn test_setup_no_test_header() {
        let mut app = create_test_service!();

        let request = test::TestRequest::post()
            .uri("/api/v1/test/verify-service")
            .to_request();

        let response = test::call_service(&mut app, request).await;

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    pub async fn find_or_create_test_db() {
        let app = create_test_service!();
        setup_test!(&app, false);
    }

    #[tokio::test]

    async fn test_validate_phone_and_email() {
        init_env_logger(log::LevelFilter::Info, log::LevelFilter::Error).await;
        let app = create_test_service!();
        let code = 569342;
        let mut proxy = TestProxy::new(&app, Some(TestContext::new(true, Some(code), None)));
        let admin_auth_token = TestHelpers::delete_all_test_users(&mut proxy).await;
        let users = TestHelpers::register_test_users(&mut proxy, Some(admin_auth_token)).await;

        let mut profile = users[0].clone();
        assert!(profile.pii.is_some());
        assert!(profile.user_id.is_some());

        // login as the first test user

        let auth_token = proxy
            .login(&profile.pii.as_ref().unwrap().email, "password")
            .await
            .expect("login should work and it should return the token");

        assert!(auth_token.len() > 0);
        proxy.set_auth_token(Some(auth_token));
        //
        //  set the phone number and email
        profile.pii.as_mut().unwrap().phone_number = SERVICE_CONFIG.test_phone_number.clone();
        profile.pii.as_mut().unwrap().email = SERVICE_CONFIG.test_email.clone();
        // update the profile
        proxy.update_profile(&profile).await.expect("success");

        //
        // we've change the email, which is encoded in the token and used as truth by the service -
        // so login again and get a new token
        let auth_token = proxy
            .login(&profile.pii.as_ref().unwrap().email, "password")
            .await
            .expect("login should work and it should return the token");
        assert!(auth_token.len() > 0);
        proxy.set_auth_token(Some(auth_token));

        // lookup profile

        let new_profile = proxy
            .get_profile("Self")
            .await
            .expect("should get a profile back from get_profile for an account that can login");

        // they better be the same!
        assert!(new_profile.is_equal_by_val(&profile));

        // send the phone code.  somebody is going to get a text...
        // the actual code is defined above and set in the test context, so that we can verify it
        proxy.send_phone_code().await.expect("success");

        //
        //  validate with the phone
        proxy.validate_phone_code(code).await.expect("success");

        //
        // send validation email
        let url_str = proxy.send_validation_email().await.expect("success");

        //  now we have to get the claim from the URL so we can pass it to the proxy
        let parts: Vec<&str> = url_str.rsplitn(2, '/').collect();
        assert_eq!(parts.len(), 2);
        let encoded_token = parts[0].clone();

        // validate with the token
        proxy.set_auth_token(None);
        proxy.validate_email(encoded_token).await.expect("success");

        // get the profile

        let profile = proxy.get_profile("Self").await.expect("success");
        assert!(profile.validated_email);
        assert!(profile.validated_phone);
        //
        // delete the users to put us back to a known state.  this is particularly important because we modified one
        // of the users and the tests can check the input vs. output of the user profile and the profile in the db will
        // be different than the profile in the .json that is used to add test users and tests will assert.
        TestHelpers::delete_all_test_users(&mut proxy).await;
    }
    #[tokio::test]
    async fn test_setup() {
        init_env_logger(log::LevelFilter::Trace, log::LevelFilter::Trace).await;
        setup_cosmos().expect("can't continue if setup fails!");
    }
}
