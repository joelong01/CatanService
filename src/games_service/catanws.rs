#![allow(dead_code)]
use crate::full_info;
use crate::middleware::service_config::SERVICE_CONFIG;
use crate::user_service::users::validate_jwt_token;
use actix::prelude::*;
use actix::{Actor, StreamHandler};
use actix_web::error::{ErrorInternalServerError, ErrorUnauthorized};
use actix_web::http::header::HeaderMap;
use actix_web::web::{Payload, Query};
use actix_web::{Error, HttpRequest, HttpResponse};
use actix_web_actors::ws;
use lazy_static::lazy_static;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

// The user context struct, you can define it based on your requirements
pub struct UserContext {
    user_id: String,
    client_address: Recipient<WebSocketMessage>,
}

impl UserContext {
    // Add any other methods and fields as needed

    // Method to send a message to the user's WebSocket
}

// Define a custom message type for sending messages to the WebSocket actor
pub struct WebSocketMessage(pub String);

// Implement the Message trait for WebSocketMessage
impl Message for WebSocketMessage {
    type Result = ();
}

// Lobby contains all connected user contexts
type Lobby = Arc<RwLock<HashMap<String, CatanWebSocket>>>;

// We'll use this to store connected users in a lazy_static Mutex
lazy_static! {
    static ref LOBBY: Lobby = Arc::new(RwLock::new(HashMap::new()));
}
#[derive(Debug, Clone)]
pub struct CatanWebSocket {
    pub user_id: String,
    pub hb: Instant,
    pub client_address: Option<Recipient<WebSocketMessage>>,
}

impl Actor for CatanWebSocket {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        // Send Connect message to LOBBY when WebSocket starts up.
        self.heart_beat(ctx);

        self.client_address = Some(ctx.address().recipient::<WebSocketMessage>());

        // Acquire a write lock to add the user to the LOBBY
        let mut lobby = LOBBY.write();
        lobby.insert(self.user_id.clone(), self.clone());
    }

    fn stopped(&mut self, _: &mut Self::Context) {
        // User WebSocket has been disconnected, remove from LOBBY

        // Acquire a write lock to remove the user from the LOBBY
        let mut lobby = LOBBY.write();
        lobby.remove(&self.user_id);
    }
}

impl CatanWebSocket {
    pub fn new(user_id: String) -> Self {
        Self {
            user_id,
            hb: Instant::now(),
            client_address: None,
        }
    }

    fn send_message(&self, message: &str) -> Result<(), ()> {
        let _ = self
            .client_address
            .as_ref()
            .unwrap()
            .send(WebSocketMessage(message.to_owned()));
        Ok(())
    }

    fn heart_beat(&self, ctx: &mut ws::WebsocketContext<Self>) {
        ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
            if Instant::now().duration_since(act.hb) > CLIENT_TIMEOUT {
                println!("Disconnecting failed heartbeat");
                ctx.stop();
                return;
            }

            ctx.ping(b"hi");
        });
    }

    // Function to send a message to the user's WebSocket
    pub fn send_client_message(user_id: &str, message: &str) -> Result<(), String> {
        // Acquire a read lock to access the LOBBY
        let lobby = LOBBY.read();

        // Get the UserContext from the LOBBY
        if let Some(client_ws) = lobby.get(user_id) {
            // Send the message to the user's WebSocket
            if let Err(_) = client_ws.send_message(message) {
                // If there was an error sending the message, return an error
                Err(format!("Failed to send message to user: {}", user_id))
            } else {
                // Message sent successfully
                Ok(())
            }
        } else {
            // User not found in the LOBBY
            Err(format!("User not connected: {}", user_id))
        }
    }
}

/// Handler for `ws::Message`
impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for CatanWebSocket {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => ctx.pong(&msg),
            Ok(ws::Message::Text(text)) => ctx.text(text),
            Ok(ws::Message::Binary(bin)) => ctx.binary(bin),
            Ok(ws::Message::Close(reason)) => ctx.close(reason),
            _ => (),
        }
    }
}
impl Handler<WebSocketMessage> for CatanWebSocket {
    type Result = ();

    fn handle(&mut self, msg: WebSocketMessage, ctx: &mut Self::Context) {
        // Handle the WebSocketMessage here (e.g., sending the message to the WebSocket)
        ctx.text(msg.0);
    }
}
pub fn dump_headers(headers: &HeaderMap) {
    // Iterate through all headers and log their name and values
    for (name, value) in headers.iter() {
        // Since the header value is stored as a comma-separated string,
        // we need to join the values to get the actual header value
        let value_str = value.to_str().unwrap_or("Invalid UTF-8");

        // Log the name and values of the header
        full_info!("Header Name: {:?}, Value: {:?}", name, value_str);
    }
}

pub async fn ws_bootstrap(
    query_params: Query<HashMap<String, String>>,
    req: HttpRequest,
    stream: Payload,
) -> Result<HttpResponse, Error> {
    // Extract the JWT token from the query parameter
    let token: String = query_params
        .get("token")
        .ok_or_else(|| ErrorUnauthorized("JWT token not provided"))?
        .to_string();

    // Validate the token and extract claims
    let claims = match validate_jwt_token(&token, &SERVICE_CONFIG.login_secret_key) {
        Some(claims) => claims.claims,
        None => return Err(ErrorUnauthorized("No Authorization Parameter")),
    };

    // Start the WebSocket handshake with the CatanWebSocket actor
    match ws::start(CatanWebSocket::new(claims.id), &req, stream) {
        Ok(_resp) => {
            // Handle the WebSocket response or any other response as needed
            // For now, we'll just return a successful response with a message indicating successful connection
            let response_body = serde_json::json!({
                "message": "connected successfully"
            });

            Ok(HttpResponse::Ok()
                .content_type("application/json")
                .json(response_body))
        }
        Err(err) => Err(ErrorInternalServerError(format!(
            "WebSocket connection failed: {:?}",
            err
        ))),
    }
}
