/**
 *  main entry point for the application.  The goal here is to set up the Web Server.
 */
mod cosmos_db;
mod games_service;
mod middleware;
mod shared;
mod user_service;

use actix::Actor;
// dependencies...
use actix_cors::Cors;
use actix_web::{
    middleware::Logger,
    web::{self, Data},
    App, HttpServer,
};

use games_service::{
    catanws::{self, Broker},
    games,
};
use middleware::authn_mw::AuthenticationMiddlewareFactory;
use middleware::environment_mw::{
    EnvironmentMiddleWareFactory, ServiceEnvironmentContext, CATAN_ENV,
};
use once_cell::sync::OnceCell;
use openssl::ssl::{SslAcceptor, SslFiletype, SslMethod};
use std::env;
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

    let broker_addr = Broker::new().start();

    //
    // set up the HttpServer - pass in the broker service as part of App data
    HttpServer::new(move || {
        App::new()
            .app_data(Data::new(ServiceEnvironmentContext::new()))
            .app_data(broker_addr.clone())
            .wrap(Logger::default())
            .wrap(EnvironmentMiddleWareFactory)
            .wrap(Cors::permissive())
            .service(
                web::scope("/api").service(
                    web::scope("/v1")
                        .route("/users/setup", web::post().to(users::setup))
                        .route("/users/register", web::post().to(users::register))
                        .route("/users/login", web::post().to(users::login)),
                ),
            )
            .service(
                web::scope("auth/api")
                    .wrap(AuthenticationMiddlewareFactory)
                    .service(
                        web::scope("/v1")
                            .route("/users", web::get().to(users::list_users))
                            .route("/users/{id}", web::delete().to(users::delete))
                            .route("/users/{id}", web::get().to(users::find_user_by_id))
                            .route("/games/{game_type}", web::post().to(games::new_game))
                            .route("/games", web::get().to(games::supported_games))
                            .route("/ws/{user_id}", web::get().to(catanws::ws_index)),
                    ),
            )
    })
    .bind_openssl(format!("0.0.0.0:{}", port), builder)?
    .run()
    .await
}
