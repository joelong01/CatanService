#![allow(unused_variables)]
use actix_web::{
    HttpRequest, HttpResponse, web,
};
use azure_core::StatusCode;

use crate::{
    games_service::game_container::game_messages::InviteData,
    user_service::users::create_http_response,
};

use super::lobby::Lobby;

pub async fn get_lobby(_req: HttpRequest) -> HttpResponse {
    return HttpResponse::Ok()
        .content_type("application/json")
        .json(Lobby::copy_lobby().await);
}
pub async fn post_invite(req: HttpRequest, invite: web::Json<InviteData>) -> HttpResponse {
    let from_id = req.headers().get("x-user-id").unwrap().to_str().unwrap();
    let invite: &InviteData = &invite;

    match Lobby::send_invite(&invite).await {
        Ok(_) => HttpResponse::Ok()
            .content_type("application/json")
            .json(Lobby::copy_lobby().await),
        Err(e) => create_http_response(
            StatusCode::BadRequest,
            format!("Error posting invite: {:?}", e),
            "".to_owned(),
        ),
    }


}

pub async fn join_game(_req: HttpRequest) -> HttpResponse {
    return HttpResponse::Ok()
        .content_type("application/json")
        .json(Lobby::copy_lobby().await);
}
