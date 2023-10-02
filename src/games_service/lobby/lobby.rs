#![allow(unused_variables)]
use crate::{
    games_service::{
        game_container::{
            game_container::GameContainer,
            game_messages::{CatanMessage, Invitation, InvitationResponseData},
        },
        long_poller::long_poller::LongPoller,
    },
    middleware::request_context_mw::RequestContext,
    shared::shared_models::{ServiceError, UserProfile}, full_info,
};

pub async fn get_lobby() -> Result<Vec<UserProfile>, ServiceError> {
   Ok(LongPoller::get_available().await)
}
pub async fn post_invite(
    from_id: &str,
    invite: &Invitation,
) -> Result<(), ServiceError> {
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
) -> Result<(), ServiceError> {
    if invite_response.accepted {
        // add the user to the Container -- now they are in both the lobby and the game
        // this will release any threads waiting for updates on the game
        let persist_user = request_context
            .database.as_user_db()
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
/*
    add a local user to a game.  it needs to be the local user of the creator (e.g. the id in the )
*/
pub async fn add_local_user(
    game_id: &str,
    local_user_id: &str,
    request_context: &RequestContext,
) -> Result<(), ServiceError> {
    let user_id = request_context
        .claims
        .as_ref()
        .expect("auth_mw should have added this or rejected the call")
        .id
        .clone();

    let local_user = request_context
        .database.as_user_db()
        .find_user_by_id(&local_user_id)
        .await?;

    match local_user.connected_user_id {
        Some(id) => {
            if id == user_id {
                let response =
                    GameContainer::add_player(&game_id, &local_user.user_profile).await?;
                return Ok(response);
            } else {
                return Err(ServiceError::new_bad_request("not your connected user!"));
            }
        }
        None => {
            return Err(ServiceError::new_bad_request("not a connected user"));
        }
    };
}

pub async fn join_lobby(
    request_context: &RequestContext,
) -> Result<(), ServiceError> {
    let user_id = request_context
        .claims
        .as_ref()
        .expect("auth_mw should have added this or rejected the call")
        .id
        .clone();

    let user = request_context
        .database.as_user_db()
        .find_user_by_id(&user_id)
        .await?;
    full_info!("connecting {}", user.user_profile.display_name);
    LongPoller::add_user(&user_id, &user.user_profile).await
}

pub async fn leave_lobby(request_context: &RequestContext) -> Result<(), ServiceError> {
    todo!()
}
