#[macro_export]
macro_rules! api_call {
    ($api_call:expr) => {{
        let result = $api_call;
        match result {
            Ok(val) => HttpResponse::Ok().json(val),
            Err(service_error) => service_error.to_http_response(),
        }
    }};
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
macro_rules! log_return_not_found {
    ( $e:expr, $msg:expr ) => {{
        log::error!("\t{}\n {:#?}", $e, $e);
        return Err(ServiceError::new(
            $msg,
            StatusCode::NOT_FOUND,
            ResponseType::ErrorInfo(format!("Error: {}", $e)),
            GameError::HttpError,
        ));
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
        use crate::middleware::request_context_mw::TestCallContext;
        use actix_web::http::header;
        use actix_web::test;
        let test_context = TestCallContext::new(None, None);
        let request = test::TestRequest::post()
            .uri("/api/v1/test/verify-service")
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
macro_rules! create_service {
    () => {{
        use crate::create_unauthenticated_service;
        use crate::AuthenticationMiddlewareFactory;
        use actix_cors::Cors;
        use actix_web::{web, App};

        use crate::{
            action_service, game_service, lobby_service, longpoll_service, profile_service,
            user_service,
        };

        use crate::middleware::request_context_mw::RequestContextMiddleware;

        App::new()
            //  .wrap(Logger::default())
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
                    .service(action_service()),
            )
    }};
}

#[macro_export]
macro_rules! create_test_service {
    () => {{
        use crate::create_service;
        use crate::init_env_logger;
        use actix_web::test;

        init_env_logger(log::LevelFilter::Trace, log::LevelFilter::Error).await;

        let app = test::init_service(create_service!()).await;
        app
    }};
}

#[macro_export]
macro_rules! full_info {
    ($($arg:tt)*) => {
        {
            let formatted_msg = format!($($arg)*);
            let cleaned_msg = crate::macros::format_log_message(&formatted_msg);
            log::info!(target: &format!("{}:{}:", file!(), line!()), "{}", cleaned_msg);
        }
    };
}

pub fn format_log_message(s: &str) -> String {
    s.replace('\n', " ") // Replace newline with space
        .split_whitespace() // Split the string by whitespace
        .collect::<Vec<&str>>() // Collect into a Vec<&str>
        .join(" ") // Join with a space
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
          log::trace!("{}:{},{},{}", file!(), line!(), $from, format!($($arg)*))
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
macro_rules! info_object_size {
    ($name:expr, $print_object:expr, $obj:expr) => {{
        use crate::full_info;
        match serde_json::to_string($obj) {
            Ok(serialized_data) => {
                let size_in_bytes = serialized_data.as_bytes().len();
                if $print_object {
                    full_info!(
                        "name: {} size: {}, object: {}",
                        $name,
                        size_in_bytes,
                        serialized_data
                    );
                } else {
                    full_info!("name: {} size: {}", $name, size_in_bytes);
                }
            }
            Err(e) => {
                full_info!("unable to serialize {}.  {:#?}", $name, e);
            }
        }
    }};
}
