/**
 *  main entry point for the application.  The goal here is to set up the Web Server.
 */
//
//  rust wants modules in the same directory declared.
mod cosmos_db;
mod games_service;
mod shared;
mod user_service;

// dependencies...
use actix_cors::Cors;
use actix_web::{web, App, HttpServer};

use games_service::{catanws, games};
use log::trace;
use once_cell::sync::OnceCell;
use openssl::ssl::{SslAcceptor, SslFiletype, SslMethod};
use shared::models::CatanSecrets;
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
                match env::var("COSMOS_RUST_SAMPLE_PORT") {
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
    //
    //   turn on max logging by setting web, server, and rust to trace level logging

    env::set_var("RUST_LOG", "actix_web=trace,actix_server=trace,rust=trace");
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let port: String = safe_set_port!();
    // this looks up env variables and puts them into a rust structt - if they aren't set, we panic
    let secrets = match CatanSecrets::load_from_env() {
        Ok(s) => {
            trace!("Secrets found.  Account: {:?}", s.cosmos_account);
            s
        },
        Err(error) => panic!(
            "Failed to get secrets: {}\n\
                              If you are running in a dev container for the \
                              first time, you need to restart VS Code.",
            error
        ),
    };

    // normally you would set the RUST_LOG environment variable in the process that this app is running in, and then
    // you'd have this code that checks for it and errors out if it doesn't exist.  this value is set above, so it will
    // always be set, but this is here for completeness
    let rust_log = env::var("RUST_LOG");
    match rust_log {
        Ok(value) => trace!("RUST_LOG: {}", value),
        Err(_) => trace!("RUST_LOG is not set"),
    }

    //
    //  SSL support
    let mut builder = SslAcceptor::mozilla_intermediate(SslMethod::tls()).unwrap();
    builder
        .set_private_key_file(secrets.ssl_key_location, SslFiletype::PEM)
        .unwrap();
    builder
        .set_certificate_chain_file(secrets.ssl_cert_location)
        .unwrap();

    //
    // set up the HttpServer

    HttpServer::new(|| {
        App::new().wrap(Cors::permissive()).service(
            web::scope("/api").service(
                web::scope("/v1")
                    .route("/users", web::get().to(users::list_users))
                    .route("/users", web::post().to(users::create))
                    .route("/users/{id}", web::delete().to(users::delete))
                    .route("/users/{id}", web::get().to(users::find_user_by_id))
                    .route("/setup", web::post().to(users::setup))
                    .route("/games/{game_type}", web::post().to(games::new_game))
                    .route("/games", web::get().to(games::supported_games))
                    .route("/ws", web::get().to(catanws::ws_index)),
            ),
        )
    })
    .bind_openssl(format!("0.0.0.0:{}", port), builder)?
    .run()
    .await
}
