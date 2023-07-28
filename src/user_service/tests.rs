#[cfg(test)]
mod tests {
    use crate::games_service::catanws;
    use crate::get_version;
    use crate::middleware::environment_mw::{ServiceEnvironmentContext, EnvironmentMiddleWareFactory};
    use crate::user_service::users;
    use actix_cors::Cors;
    use actix_web::http::{header, StatusCode};
    use actix_web::middleware::Logger;
    use actix_web::web::Data;
    use actix_web::{test, web, App};

    #[actix_rt::test]
    async fn test_setup_no_test_header() {
        let mut app = test::init_service(
            App::new()
                .wrap(Logger::default())
                .service(
                    web::scope("api/ws")
                        .route("/wsbootstrap", web::get().to(catanws::ws_bootstrap)),
                )
                .app_data(Data::new(ServiceEnvironmentContext::new()))
                .wrap(EnvironmentMiddleWareFactory)
                .wrap(Cors::permissive())
                .service(
                    web::scope("/api").service(
                        web::scope("/v1")
                            .route("/version", web::get().to(get_version))
                            .route("/users/setup", web::post().to(users::setup))
                            .route("/users/register", web::post().to(users::register))
                            .route("/users/login", web::post().to(users::login)),
                    ),
                ),
        )
        .await;

        let request = test::TestRequest::post()
            .uri("/api/v1/users/setup")
            .to_request();

        let response = test::call_service(&mut app, request).await;

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[actix_rt::test]
    async fn test_setup_with_test_header() {
        let mut app = test::init_service(
            App::new()
                .wrap(Logger::default())
                .service(
                    web::scope("api/ws")
                        .route("/wsbootstrap", web::get().to(catanws::ws_bootstrap)),
                )
                .app_data(Data::new(ServiceEnvironmentContext::new()))
                .wrap(EnvironmentMiddleWareFactory)
                .wrap(Cors::permissive())
                .service(
                    web::scope("/api").service(
                        web::scope("/v1")
                            .route("/version", web::get().to(get_version))
                            .route("/users/setup", web::post().to(users::setup))
                            .route("/users/register", web::post().to(users::register))
                            .route("/users/login", web::post().to(users::login)),
                    ),
                ),
        )
        .await;

        let request = test::TestRequest::post()
            .uri("/api/v1/users/setup")
            .append_header((header::CONTENT_TYPE, "application/json"))
            .append_header(("is_test", "true"))
            .to_request();

        let response = test::call_service(&mut app, request).await;

        assert_eq!(response.status(), StatusCode::OK);
    }

    // Add more test functions as needed

  
}
