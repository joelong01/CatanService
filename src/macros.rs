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
    ($app:expr) => {{
        use actix_web::http::header;
        use actix_web::test;

        let request = test::TestRequest::post()
            .uri("/api/v1/test/setup")
            .append_header((header::CONTENT_TYPE, "application/json"))
            .append_header((GameHeaders::IS_TEST, "true"))
            .to_request();

        let response = test::call_service($app, request).await;
        assert!(response.status().is_success());
    }};
}

#[macro_export]
macro_rules! create_app {
    () => {{
        use crate::AuthenticationMiddlewareFactory;
        use crate::EnvironmentMiddleWareFactory;
        use actix_cors::Cors;
        use actix_web::{middleware::Logger, web, App};
        use crate::create_unauthenticated_service;


        use crate::ServiceEnvironmentContext;
        use crate::{game_service, lobby_service, longpoll_service, profile_service, user_service};
        use actix_web::web::Data;
        App::new()
            .wrap(Logger::default())
            .app_data(Data::new(ServiceEnvironmentContext::new()))
            .wrap(EnvironmentMiddleWareFactory)
            .wrap(Cors::permissive())
            .service(create_unauthenticated_service()) // Make sure this function is in scope
            .service(
                web::scope("auth/api/v1")
                    .wrap(AuthenticationMiddlewareFactory)
                    .service(user_service())
                    .service(lobby_service())
                    .service(game_service())
                    .service(longpoll_service())
                    .service(profile_service()), // Make sure this function is in scope
            )
    }};
}

#[macro_export]
macro_rules! create_test_service {
    () => {{
        use crate::create_app;
        use actix_web::test;
        use crate::init_env_logger;

        init_env_logger().await;

        let app = test::init_service(create_app!()).await;
        app
    }};
}
