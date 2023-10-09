use actix_web::{dev::Payload, error::Error, FromRequest, HttpRequest, Result};
use futures::future::{ok, Ready};
use serde::Serialize;

use crate::{
    games_service::game_container::game_messages::GameHeader,
    shared::shared_models::{LoginHeaderData, ProfileStorage},
};

use super::request_context_mw::TestCallContext;

// Custom extractor for multiple headers
#[derive(Serialize)]
pub struct HeadersExtractor {
    pub game_id: Option<String>,
    pub login_data: Option<LoginHeaderData>,
    pub password: Option<String>, // needed for register_user
    pub profile_storage: Option<ProfileStorage>,
    pub test_call_context: Option<TestCallContext>,
}

impl FromRequest for HeadersExtractor {
    type Error = Error;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        let headers = req.headers();

        let game_id = headers
            .get(GameHeader::GAME_ID)
            .and_then(|v| v.to_str().ok().map(String::from));

        let password = headers
            .get(GameHeader::PASSWORD)
            .and_then(|v| v.to_str().ok().map(String::from));

        let login_data = headers
            .get(GameHeader::LOGIN_DATA)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| serde_json::from_str::<LoginHeaderData>(s).ok());

        let test_call_context = headers
            .get(GameHeader::TEST)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| serde_json::from_str::<TestCallContext>(s).ok());

        let profile_storage = headers
            .get(GameHeader::PROFILE_LOCATION)
            .and_then(|v| v.to_str().ok()) // Converts HeaderValue to Option<&str>
            .and_then(|s| serde_json::from_str::<ProfileStorage>(s).ok()) // Converts &str to Option<ProfileStorage>
            .unwrap_or(ProfileStorage::CosmosDb); // If None at any step, use default

        // Return the extracted values
        let header_extractor = HeadersExtractor {
            game_id,
            test_call_context,
            login_data,
            password,
            profile_storage: Some(profile_storage),
        };
        //
        //  be careful logging this information as it has clear text passwords...
        // let json = serde_json::to_string(&header_extractor).unwrap_or_else(|_| "error serializing headers?".to_string());
        // full_info!("headers: {}", json);
        ok(header_extractor)
    }
}

///
/// this macro is designed to be run from the *handlers APIs to get the underlying data. if the header isn't set
/// it will return a bad request HTTP error
#[macro_export]
macro_rules! get_header_value {
    ($header:ident, $headers:expr) => {{
        use crate::shared::shared_models::ServiceError;

        match $headers.$header {
            Some(v) => v,
            None => {
                let msg = format!("{} header not found", stringify!($header));
                let response = ServiceError::new_bad_request(&msg);
                return response.to_http_response();
            }
        }
    }};
}
