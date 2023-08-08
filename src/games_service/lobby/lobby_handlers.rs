#![allow(unused_variables)]
use actix_web::{
    web::{self, Data},
    HttpRequest, HttpResponse,
};
use azure_core::StatusCode;

use crate::{
    games_service::{
        game_container::{
            game_container::GameContainer,
            game_messages::{CatanMessage, GameHeader, Invitation, InvitationResponseData},
        },
        long_poller::long_poller::LongPoller,
    },
    middleware::environment_mw::ServiceEnvironmentContext,
    shared::models::ClientUser,
    trace_function,
    user_service::users::{create_http_response, internal_find_user},
};


pub async fn get_lobby(_req: HttpRequest) -> HttpResponse {
    return HttpResponse::Ok()
        .content_type("application/json")
        .json(LongPoller::get_available().await);
}
pub async fn post_invite(req: HttpRequest, invite: web::Json<Invitation>) -> HttpResponse {
    let from_id = req
        .headers()
        .get(GameHeader::USER_ID)
        .unwrap()
        .to_str()
        .unwrap();
    let invite: &Invitation = &invite;

    match LongPoller::send_message(
        vec![invite.to_id.clone()],
        &CatanMessage::Invite(invite.clone()),
    )
    .await
    {
        Ok(_) => create_http_response(StatusCode::Ok, "", ""),
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
        .await
        .expect("add_player shouldn't fail.  TODO: handle failure");
    }

    // tell both the sender and reciever the result of the invitation

    if let Err(e) = LongPoller::send_message(
        vec![
            invite_response.from_id.clone(),
            invite_response.to_id.clone(),
        ],
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

    create_http_response(StatusCode::Accepted, "accepted", "")
}
