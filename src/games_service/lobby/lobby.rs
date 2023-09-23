#![allow(unused_variables)]
use reqwest::StatusCode;

use crate::{
    games_service::{
        game_container::{
            game_container::GameContainer,
            game_messages::{CatanMessage, Invitation, InvitationResponseData},
        },
        long_poller::long_poller::LongPoller,
    },
    middleware::request_context_mw::RequestContext,
    shared::shared_models::{UserProfile, GameError, ResponseType, ServiceResponse},
};

pub async fn get_lobby() -> Result<ServiceResponse, ServiceResponse> {
    return Ok(ServiceResponse::new(
        "",
        StatusCode::OK,
        ResponseType::Profiles(LongPoller::get_available().await),
        GameError::NoError(String::default()),
    ));
}
pub async fn post_invite(
    from_id: &str,
    invite: &Invitation,
) -> Result<ServiceResponse, ServiceResponse> {
    LongPoller::send_message(
        vec![invite.to_id.clone()],
        &CatanMessage::Invite(invite.clone()),
    )
    .await
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
    is_test: bool,
    invite_response: &InvitationResponseData,
    request_context: &RequestContext,
) -> Result<ServiceResponse, ServiceResponse> {
    if invite_response.accepted {
        // add the user to the Container -- now they are in both the lobby and the game
        // this will release any threads waiting for updates on the game
        let persist_user = request_context
            .database
            .find_user_by_id(&invite_response.from_id)
            .await?;
        GameContainer::add_player(
            &invite_response.game_id,
            &UserProfile::from_persist_user(&persist_user),
        )
        .await
        .expect("add_player shouldn't fail.  TODO: handle failure");
    }

    // tell the reciever the result of the invitation

    LongPoller::send_message(
        vec![invite_response.to_id.clone()],
        &CatanMessage::InvitationResponse(invite_response.clone()),
    )
    .await
}
