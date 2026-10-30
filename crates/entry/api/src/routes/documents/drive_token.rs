//! `POST /v1/bridge/drive/token`: the Drive sync's own credential.
//!
//! The device trades its private device token (audience
//! [`BRIDGE_DEVICE_AUDIENCE`] alone) for a short token whose only audience is
//! [`DRIVE_SYNC_AUDIENCE`]; the Drive routes accept that token and nothing
//! else, and only it is stamped with the `drive_sync` channel. The device
//! token is minted beside the bridge token for the bridge client with no
//! purpose and kept by the bridge; what Claude Code, the MCP proxy and every
//! other host receive is the bridge token, which has no device audience and
//! so cannot trade. A purposed or delegated token is refused too. The new
//! token carries the device token's user, session and `interaction` claim,
//! never more, and expires no later than it. Its only permission is `user`,
//! the one the Drive routes act on, whatever else the device token held.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Json;
use axum::extract::State;
use axum::http::HeaderMap;
use systemprompt_files::documents::agents::{AgentChannel, DocumentActor};
use systemprompt_identifiers::JwtToken;
use systemprompt_models::api::ApiError;
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::DriveSyncToken;
use systemprompt_oauth_issuance::drive_sync::{device_audiences, drive_audiences};
use systemprompt_runtime::documents::actor::bearer_actor;

use super::caller::authenticated;
use super::drive::DriveState;
use super::error::DocumentsResult;
use crate::error::ApiHttpError;
use crate::routes::gateway::messages::extract_credential;

pub(super) async fn issue(
    State(state): State<DriveState>,
    headers: HeaderMap,
) -> DocumentsResult<Json<DriveSyncToken>> {
    let credential = extract_credential(&headers)
        .ok_or_else(|| ApiHttpError::unauthorized("the trade takes the bridge device token"))?;
    let (claims, user) = state
        .jwt()
        .decode_for_audiences(&JwtToken::new(credential), &device_audiences())
        .await
        .map_err(|error| {
            ApiError::unauthorized("only the bridge device token trades for a Drive sync token")
                .with_source(error)
        })?;
    let token = state
        .drive_sync()
        .exchange(&claims, user)
        .map_err(ApiHttpError::from)?;
    Ok(Json(token))
}

pub(super) async fn device_user(
    state: &DriveState,
    headers: &HeaderMap,
) -> DocumentsResult<(AuthenticatedUser, DocumentActor)> {
    let credential = extract_credential(headers)
        .ok_or_else(|| ApiHttpError::unauthorized("the Drive routes take the Drive sync token"))?;
    let (claims, user) = state
        .jwt()
        .decode_for_audiences(&JwtToken::new(credential), &drive_audiences())
        .await
        .map_err(|error| {
            ApiError::unauthorized("the Drive routes take only the Drive sync token")
                .with_source(error)
        })?;
    if claims.purpose.is_some() || !claims.act_chain.is_empty() {
        return Err(ApiHttpError::forbidden("a purposed or delegated token cannot sync").into());
    }
    let actor = bearer_actor(None, claims.client_id.as_ref(), claims.interaction);
    Ok((authenticated(user), actor.on(AgentChannel::DriveSync)))
}
