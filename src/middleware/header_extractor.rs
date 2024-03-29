use actix_web::{dev::Payload, error::Error, FromRequest, HttpRequest, Result};
use futures::future::{ok, Ready};

use crate::games_service::game_container::game_messages::GameHeader;

// Custom extractor for multiple headers
pub struct HeadersExtractor {
    pub game_id: Option<String>,
    pub user_id: Option<String>,
    pub password: Option<String>,
    pub is_test: bool,
    pub email: Option<String>,
}

impl FromRequest for HeadersExtractor {
    type Error = Error;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        let headers = req.headers();

        let game_id = headers
            .get(GameHeader::GAME_ID)
            .and_then(|v| v.to_str().ok().map(String::from));
        let user_id = headers
            .get(GameHeader::USER_ID)
            .and_then(|v| v.to_str().ok().map(String::from));
        let password = headers
            .get(GameHeader::PASSWORD)
            .and_then(|v| v.to_str().ok().map(String::from));
        let is_test = headers.contains_key(GameHeader::TEST);
        let email = headers
            .get(GameHeader::EMAIL)
            .and_then(|v| v.to_str().ok().map(String::from));

        // Return the extracted values
        ok(HeadersExtractor {
            game_id,
            user_id,
            password,
            is_test,
            email,
        })
    }
}
#[macro_export]
macro_rules! get_header_value {
    ($header:ident, $headers:expr) => {{
        use crate::shared::shared_models::{GameError, ResponseType, ServiceResponse};
        use reqwest::StatusCode;
        match $headers.$header {
            Some(v) => v,
            None => {
                let msg = format!("{} header not found", stringify!($header));
                let response = ServiceResponse::new(
                    &msg,
                    reqwest::StatusCode::BAD_REQUEST,
                    ResponseType::NoData,
                    GameError::HttpError(StatusCode::BAD_REQUEST),
                );
                return HttpResponse::BadRequest().json(response);
            }
        }
    }};
}
