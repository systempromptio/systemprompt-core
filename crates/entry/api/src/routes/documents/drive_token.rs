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
use chrono::{Duration, TimeZone, Utc};
use systemprompt_files::documents::agents::{AgentChannel, DocumentActor};
use systemprompt_identifiers::{AccessTokenId, ClientId, JwtToken};
use systemprompt_models::api::ApiError;
use systemprompt_models::auth::{AuthenticatedUser, JwtAudience, Permission, UserJwtRequest};
use systemprompt_models::documents::{BRIDGE_DEVICE_AUDIENCE, DRIVE_SYNC_AUDIENCE, DriveSyncToken};
use systemprompt_runtime::documents::actor::bearer_actor;

use super::caller::authenticated;
use super::drive::DriveState;
use super::error::DocumentsResult;
use crate::error::ApiHttpError;
use crate::routes::gateway::messages::extract_credential;

const MAX_TTL: Duration = Duration::hours(1);

fn device_audiences() -> [JwtAudience; 1] {
    [JwtAudience::Resource(BRIDGE_DEVICE_AUDIENCE.to_owned())]
}

#[must_use]
pub(super) fn drive_audiences() -> [JwtAudience; 1] {
    [JwtAudience::Resource(DRIVE_SYNC_AUDIENCE.to_owned())]
}

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
    let bridge_client = claims.client_id.as_ref() == Some(&ClientId::bridge());
    if !bridge_client || claims.purpose.is_some() || !claims.act_chain.is_empty() {
        return Err(ApiHttpError::forbidden(
            "only the bridge device's own token can start a Drive sync",
        )
        .into());
    }
    let bridge_expiry = Utc
        .timestamp_opt(claims.exp, 0)
        .single()
        .unwrap_or_else(Utc::now);
    let ttl = (bridge_expiry - Utc::now()).min(MAX_TTL);
    if ttl <= Duration::zero() {
        return Err(ApiHttpError::unauthorized("the bridge token has expired").into());
    }
    let who = authenticated(user);
    if !who.has_permission(Permission::User) {
        return Err(ApiHttpError::forbidden("a Drive sync needs a user account").into());
    }
    let request = UserJwtRequest {
        permissions: vec![Permission::User],
        user: who,
        session_id: claims.session_id.clone(),
        jti: AccessTokenId::generate(),
        audience: drive_audiences().to_vec(),
        ttl,
        client_id: Some(ClientId::bridge()),
        purpose: None,
        plugin_id: None,
        interaction: claims.interaction,
        browser_session: false,
    };
    let token = state
        .tokens()
        .mint_user_jwt(&request)
        .map_err(|error| ApiHttpError::internal("drive sync token was not minted", error))?;
    Ok(Json(DriveSyncToken {
        token,
        expires_at: Utc::now() + ttl,
    }))
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
    let actor = bearer_actor(
        claims.purpose.as_deref(),
        claims.client_id.as_ref(),
        claims.interaction,
    );
    Ok((authenticated(user), actor.on(AgentChannel::DriveSync)))
}
