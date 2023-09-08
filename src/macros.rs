#[macro_export]
macro_rules! serialize_as_array2 {
    ($key:ty, $value:ty) => {
        fn serialize_as_array<S>(
            data: &HashMap<$key, $value>,
            serializer: S,
        ) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            let values: Vec<$value> = data.values().cloned().collect();
            values.serialize(serializer)
        }
    };
}
#[macro_export]
macro_rules! deserialize_from_array {
    ($key:ty, $value:ty) => {
        fn deserialize_from_array<'de, D>(
            deserializer: D,
        ) -> Result<HashMap<$key, $value>, D::Error>
        where
            D: Deserializer<'de>,
        {
            let values: Vec<$value> = Vec::deserialize(deserializer)?;
            let mut map = HashMap::new();

            for value in values {
                map.insert(value.key.clone(), value);
            }

            Ok(map)
        }
    };
}

#[macro_export]
macro_rules! log_return_err {
    ( $e:expr ) => {{
        log::error!("\t{}\n {:#?}", $e, $e);
        return Err($e);
    }};
}

#[macro_export]
macro_rules! serialize_as_array {
    ($key:ty, $value:ty) => {
        fn serialize_as_array_impl<S>(
            data: &HashMap<$key, $value>,
            serializer: S,
        ) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            let values: Vec<$value> = data.values().cloned().collect();
            values.serialize(serializer)
        }

        serialize_as_array_impl
    };
}

#[macro_export]
macro_rules! setup_test {
    ($app:expr, $use_cosmos_db:expr) => {{
        use actix_web::http::header;
        use actix_web::test;

        let test_context = TestContext::new($use_cosmos_db);
        let request = test::TestRequest::post()
            .uri("/api/v1/test/setup")
            .append_header((header::CONTENT_TYPE, "application/json"))
            .append_header((
                GameHeader::TEST,
                serde_json::to_string(&test_context).expect("JSON should serialize!"),
            ))
            .to_request();

        let response = test::call_service($app, request).await;
        assert!(response.status().is_success());
    }};
}


#[macro_export]
macro_rules! create_app {
    () => {{
        use crate::create_unauthenticated_service;
        use crate::AuthenticationMiddlewareFactory;
        use actix_cors::Cors;
        use actix_web::{middleware::Logger, web, App};


        use crate::{game_service, lobby_service, longpoll_service, profile_service, user_service, action_service};

        use crate::middleware::environment_mw::RequestContextMiddleware;
        
        App::new()
            .wrap(Logger::default())
            .wrap(RequestContextMiddleware)
            .wrap(Cors::permissive())
            .service(create_unauthenticated_service()) // Make sure this function is in scope
            .service(
                web::scope("auth/api/v1")
                    .wrap(AuthenticationMiddlewareFactory)
                    .service(user_service())
                    .service(lobby_service())
                    .service(game_service())
                    .service(longpoll_service())
                    .service(profile_service())
                    .service(action_service())
                    
            )
    }};
}

#[macro_export]
macro_rules! create_test_service {
    () => {{
        use crate::create_app;
        use crate::init_env_logger;
        use actix_web::test;

        init_env_logger(log::LevelFilter::Trace, log::LevelFilter::Error).await;

        let app = test::init_service(create_app!()).await;
        app
    }};
}

#[macro_export]
macro_rules! full_info {
    ($($arg:tt)*) => {
        log::info!(target: &format!("{}:{}:", file!(), line!()), $($arg)*)
    };
}

#[macro_export]
macro_rules! log_thread_info {
    ($from:expr, $($arg:tt)*) => {
        log::info!("[{}]:{},[{}:{}]", $from, { format!($($arg)*).replace("\n", "").replace("  ", "") }, file!(), line!())
    };
}


#[macro_export]
macro_rules! trace_thread_info {

    ($from:expr, $($arg:tt)*) => {{
       //  log::trace!("{}:{},{},{}", file!(), line!(), $from, format!($($arg)*))
    }};
}
#[macro_export]
macro_rules! trace_function {
    ($function:expr) => {{
        use scopeguard::defer;
        use std::time::Instant;
        
        let enter_time = Instant::now();
        println!("Entering {}", $function);
        
        defer! {
            let elapsed = enter_time.elapsed();
            let duration_in_nanos = elapsed.as_secs() * 1_000_000_000 + elapsed.subsec_nanos() as u64;
            println!("Leaving {} after duration: {} nanoseconds", $function, duration_in_nanos);
        }
    }};
}
#[macro_export]
macro_rules! crack_game_update {
    ($message:expr) => {
        match $message {
            CatanMessage::GameUpdate(regular_game) => Ok(regular_game),
            _ => Err("Expected GameUpdate variant"),
        }
    };
}
#[macro_export]
macro_rules! crack_game_created {
    ($message:expr) => {
        match $message {
            CatanMessage::GameCreated(data) => Ok(data),
            _ => Err("Expected GameCreated variant"),
        }
    };
}
