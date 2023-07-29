#![allow(unused_variables)]
use actix_web::{HttpRequest, HttpResponse, web::Path, };

use super::lobby::Lobby;

pub async fn get_lobby(_req: HttpRequest) -> HttpResponse {
    return HttpResponse::Ok()
        .content_type("application/json")
        .json(Lobby::copy_lobby().await);
}
pub async fn post_invite(req: HttpRequest, to_id: Path<String>,) -> HttpResponse {
    let from_id = req.headers().get("X:user_id").unwrap().to_str().unwrap();
    let to_id: &str = &to_id;

    // match Lobby::send_invite(from_id, to_id) {
    //     Ok()
    // }

    return HttpResponse::Ok()
        .content_type("application/json")
        .json(Lobby::copy_lobby().await);
}

pub async fn join_game(_req: HttpRequest) -> HttpResponse {
    return HttpResponse::Ok()
        .content_type("application/json")
        .json(Lobby::copy_lobby().await);
}
