use actix::{Actor, StreamHandler};
use actix_web::{web, Error, HttpRequest, HttpResponse};
use actix_web_actors::ws;

use actix::prelude::*;
use std::collections::HashMap;

use actix::SystemService;
impl Supervised for Broker {}
impl SystemService for Broker {
    fn service_started(&mut self, _ctx: &mut Context<Self>) {
        println!("Broker started!");
    }
}

/// Define broker
pub struct Broker {
    // Map of User IDs to WebSocket addresses.
    clients: HashMap<String, Addr<CatanWebSocket>>,
}
impl Broker {
    pub fn new() -> Self {
        Broker {
            clients: HashMap::new(),
        }
    }
}
impl Default for Broker {
    fn default() -> Broker {
        Broker {
            clients: HashMap::new(),
        }
    }
}

impl Actor for Broker {
    type Context = Context<Self>;
}
/// Define Connect message
pub struct Connect {
    pub id: String,
    pub addr: Addr<CatanWebSocket>,
}

impl Message for Connect {
    type Result = ();
}

impl Handler<Connect> for Broker {
    type Result = ();

    fn handle(&mut self, msg: Connect, _ctx: &mut Context<Self>) {
        self.clients.insert(msg.id, msg.addr);
    }
}

/// Define HTTP actor
/// Define HTTP actor
pub struct CatanWebSocket {
    pub id: String,
    pub broker: Addr<Broker>,
}

impl Actor for CatanWebSocket {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        // Send Connect message to Broker when WebSocket starts up.
        let addr = ctx.address();
        Broker::from_registry().do_send(Connect {
            id: self.id.clone(),
            addr: addr.clone(),
        });
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

pub async fn ws_index(
    req: HttpRequest,
    stream: web::Payload,
    broker: web::Data<Addr<Broker>>,
) -> Result<HttpResponse, Error> {
    let user_id: String = req.match_info().get("user_id").unwrap().parse().unwrap();
    let resp = ws::start(
        CatanWebSocket {
            id: user_id,
            broker: broker.get_ref().clone(),
        },
        &req,
        stream,
    );
    println!("{:?}", resp);
    resp
}

//
//

// define in your messages section
pub struct ClientMessage {
    pub id: String,
    pub msg: String,
}

impl Message for ClientMessage {
    type Result = ();
}

impl Handler<ClientMessage> for Broker {
    type Result = ();

    fn handle(&mut self, msg: ClientMessage, _ctx: &mut Self::Context) {
        if let Some(addr) = self.clients.get(&msg.id) {
            addr.do_send(ServerMessage {
                msg: msg.msg.clone(),
            });
        }
    }
}

pub async fn send_client_message(broker_addr: Addr<Broker>, id: String, msg: String) {
    broker_addr.do_send(ClientMessage { id, msg });
}

pub struct BroadcastMessage {
    pub msg: String,
}

impl Message for BroadcastMessage {
    type Result = ();
}

impl Handler<BroadcastMessage> for Broker {
    type Result = ();

    fn handle(&mut self, msg: BroadcastMessage, _ctx: &mut Self::Context) {
        for addr in self.clients.values() {
            addr.do_send(ServerMessage {
                msg: msg.msg.clone(),
            });
        }
    }
}

pub async fn broadcast_message(broker_addr: Addr<Broker>, msg: String) {
    broker_addr.do_send(BroadcastMessage { msg });
}

pub struct ServerMessage {
    pub msg: String,
}

impl Message for ServerMessage {
    type Result = ();
}

impl Handler<ServerMessage> for CatanWebSocket {
    type Result = ();

    fn handle(&mut self, msg: ServerMessage, ctx: &mut Self::Context) {
        ctx.text(msg.msg);
    }
}
