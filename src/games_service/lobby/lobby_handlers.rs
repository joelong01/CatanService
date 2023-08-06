#![allow(unused_variables)]
use actix_web::{web, HttpRequest, HttpResponse};
use azure_core::StatusCode;

use crate::{
    games_service::game_container::{
        game_container::GameContainer,
        game_messages::{CatanMessage, GameHeaders, Invitation, InvitationResponseData},
    },
    user_service::users::create_http_response,
};

use super::lobby::Lobby;

pub async fn get_lobby(_req: HttpRequest) -> HttpResponse {
    return HttpResponse::Ok()
        .content_type("application/json")
        .json(Lobby::copy_lobby().await);
}
pub async fn post_invite(req: HttpRequest, invite: web::Json<Invitation>) -> HttpResponse {
    let from_id = req
        .headers()
        .get(GameHeaders::USER_ID)
        .unwrap()
        .to_str()
        .unwrap();
    let invite: &Invitation = &invite;

    match Lobby::send_message(&invite.recipient_id, &CatanMessage::Invite(invite.clone())).await {
        Ok(_) => HttpResponse::Ok()
            .content_type("application/json")
            .json(Lobby::copy_lobby().await),
        Err(e) => create_http_response(
            StatusCode::BadRequest,
            &format!("Error posting invite: {:?}", e),
            "",
        ),
    }
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
    req: HttpRequest,
    invite_response: web::Json<InvitationResponseData>,
) -> HttpResponse {
    let invite_response = invite_response.into_inner();
    if invite_response.accepted {
        // move the user
        Lobby::leave_lobby(&invite_response.recipient_id).await;
        GameContainer::add_player(&invite_response.game_id, &invite_response.recipient_id).await;
        // tell the long poller of the caller (e.g. the recepient_id) that the invitation was accepted
        if let Err(e) = Lobby::send_message(
            &invite_response.recipient_id,
            &CatanMessage::InvitationResponse(invite_response.clone()),
        )
        .await
        {
            return create_http_response(
                StatusCode::BadRequest,
                &format!("Error in sending message to lobby, {:#?}", e),
                "",
            );
        }
    }

    //
    //  tell whoever sent the message the answer to the invite
    if let Err(e) = Lobby::send_message(
        &invite_response.originator_id,
        &CatanMessage::InvitationResponse(invite_response.clone()),
    )
    .await
    {
        return create_http_response(
            StatusCode::BadRequest,
            &format!("Error in sending message to lobby, {:#?}", e),
            "",
        );
    }

    //
    // if the invitation was rejected we can leave the long_poller of the recipient waiting on the lobby

    create_http_response(StatusCode::Accepted, "accepted", "")
}
