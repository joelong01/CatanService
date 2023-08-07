#![allow(unused_variables)]
use actix_web::{
    web::{self, Data},
    HttpRequest, HttpResponse,
};
use azure_core::StatusCode;

use crate::{
    games_service::game_container::{
        game_container::GameContainer,
        game_messages::{CatanMessage, GameHeader, Invitation, InvitationResponseData},
    },
    middleware::environment_mw::ServiceEnvironmentContext,
    shared::models::ClientUser,
    trace_function,
    user_service::users::{create_http_response, internal_find_user},
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
        .get(GameHeader::USER_ID)
        .unwrap()
        .to_str()
        .unwrap();
    let invite: &Invitation = &invite;

    match Lobby::send_message(&invite.to_id, &CatanMessage::Invite(invite.clone())).await {
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
    data: Data<ServiceEnvironmentContext>,
) -> HttpResponse {
    let invite_response = invite_response.into_inner();

    trace_function!("respond_to_invite", "invitation: {:?}", invite_response);
    if invite_response.accepted {
        // add the user to the Container -- now they are in both the lobby and the game
        // this will release any threads waiting for updates on the game
        let is_test = req.headers().contains_key(GameHeader::IS_TEST);
        let persist_user = internal_find_user("id", &invite_response.from_id, is_test, &data).await;
        if persist_user.is_err() {
            return create_http_response(
                StatusCode::NotFound,
                &format!("could not find user with id {}", invite_response.from_id),
                "",
            );
        }
        let persist_user = persist_user.unwrap();
        GameContainer::add_player(
            &invite_response.game_id,
            &ClientUser::from_persist_user(persist_user),
        )
        .await.expect("add_player shouldn't fail.  TODO: handle failure");

        //
        // tell the long poller of the *sender* that they accepted the message -- this way the sender's long
        // poller gets the game_id

        // tell the long poller of the caller (e.g. the from_id) that the invitation was accepted
        if let Err(e) = Lobby::send_message(
            &invite_response.from_id,
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

   // the creator of the game has a game_id, so they will be waiting on the GameContainer, 
   // so we don't send a message there. the act above of GameContainer::add_player() will 
   // update that thread -- TODO: we do need a way to tell the client "user declined"

    create_http_response(StatusCode::Accepted, "accepted", "")
}
