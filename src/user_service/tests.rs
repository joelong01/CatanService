#[cfg(test)]
mod tests {
    use crate::user_service::users;
    use actix_web::http::{header, StatusCode};
    use actix_web::{test, web, App};

    #[actix_rt::test]
    async fn test_setup_no_test_header() {
        let mut app = test::init_service(
            App::new()
                .service(web::scope("/api/v1").route("/users/setup", web::post().to(users::setup))),
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
            App::new().route("api/v1/users/setup", web::post().to(users::setup)),
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

    // Integration test that runs all the tests
    #[actix_rt::test]
    async fn test_all() {
        test_setup_no_test_header();
        test_setup_with_test_header();
        // Call other test functions here
    }
}
