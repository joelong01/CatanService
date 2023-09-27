#![allow(unused_variables)]
use actix_web::{web, HttpRequest, HttpResponse};

use crate::{
    bad_request_from_string,
    games_service::game_container::game_messages::{Invitation, InvitationResponseData},
    middleware::{header_extractor::HeadersExtractor, request_context_mw::RequestContext},
};

pub async fn get_lobby(_req: HttpRequest) -> HttpResponse {
    super::lobby::get_lobby()
        .await
        .map(|sr| sr.to_http_response())
        .unwrap_or_else(|sr| sr.to_http_response())
}
pub async fn post_invite(
    headers: HeadersExtractor,
    invite: web::Json<Invitation>,
    request_context: RequestContext,
) -> HttpResponse {
    let from_id = &request_context
        .claims
        .as_ref()
        .expect("auth_mw should set this for all authenticated APIs")
        .id;
    let invite: &Invitation = &invite;

    super::lobby::post_invite(&from_id, invite)
        .await
        .map(|sr| sr.to_http_response())
        .unwrap_or_else(|sr| sr.to_http_response())
}
/**
 * pass this on to the client.  the long_poll will pass it to the client, which will update the UI to indicate this
 * player has accepted.  
 *  1. if the invitation is accepted, move them from the lobby to the game
 *  2. notify the originator of the answer
 *  3. notify the sender (e.g. the reciever of the original invite) that a response has occured so that it will
 *     loop and end up waiting on the right thing
 */
pub async fn respond_to_invite(
    headers: HeadersExtractor,
    invite_response: web::Json<InvitationResponseData>,
    request_context: RequestContext,
) -> HttpResponse {
    let invite_response = invite_response.into_inner();
    let is_test = headers.is_test;
    super::lobby::respond_to_invite(is_test, &invite_response, &request_context)
        .await
        .map(|sr| sr.to_http_response())
        .unwrap_or_else(|sr| sr.to_http_response())
}

pub async fn add_local_user_handler(
    local_user_id: web::Path<String>,
    headers: HeadersExtractor,
    request_context: RequestContext,
) -> HttpResponse {
    let game_id = if let Some(id) = headers.game_id {
        id
    } else {
        return bad_request_from_string!("missing gameid header").to_http_response();
    };
    super::lobby::add_local_user(&game_id, &local_user_id, &request_context)
        .await
        .map(|sr| sr.to_http_response())
        .unwrap_or_else(|sr| sr.to_http_response())
}

pub async fn connect_handler(
    request_context: RequestContext,
) -> HttpResponse {
   
    super::lobby::connect(&&request_context)
        .await
        .map(|sr| sr.to_http_response())
        .unwrap_or_else(|sr| sr.to_http_response())
}

pub async fn disconnect_handler(
    request_context: RequestContext,
) -> HttpResponse {

    super::lobby::disconnect(&&request_context)
        .await
        .map(|sr| sr.to_http_response())
        .unwrap_or_else(|sr| sr.to_http_response())

}
