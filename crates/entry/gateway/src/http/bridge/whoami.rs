//! `GET /v1/bridge/whoami` — identity envelope for the bridge profile tab.
//!
//! Decodes the bearer JWT, looks up the user record for email / display name
//! / roles, resolves the user's subject attributes from the bound source, and
//! returns the subset the gateway can authoritatively answer.
//! Fields the gateway has no source for (`tenant_id`, `provider`) are not
//! emitted; the bridge falls back to its locally verified identity snapshot
//! for those.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Json;
use axum::http::HeaderMap;
use std::collections::BTreeMap;
use systemprompt_identifiers::UserId;
use systemprompt_models::authz::{SubjectAttributeSource, SubjectSourceError};
use systemprompt_models::bridge::gateway::WhoamiResponse;

use super::BridgeRouteState;
use super::error::{BridgeError, authenticate_bridge};
use systemprompt_extension::router::ApiHttpError;

pub async fn handle(
    state: BridgeRouteState,
    headers: HeaderMap,
) -> Result<Json<WhoamiResponse>, ApiHttpError> {
    let (claims, _user) = authenticate_bridge(&state.authenticator, &headers).await?;

    let user = state
        .repos
        .users
        .find_by_id(&claims.user_id)
        .await
        .map_err(BridgeError::from)?
        .ok_or_else(|| BridgeError::UserNotFound(claims.user_id.clone()))?;

    let attributes = subject_attributes(&state, &claims.user_id).await;

    Ok(Json(WhoamiResponse {
        user_id: Some(claims.user_id),
        email: Some(user.email),
        display_name: user.display_name.or(user.full_name),
        roles: user.roles,
        attributes,
        ..WhoamiResponse::default()
    }))
}

// Why: the attributes describe the user to a profile view; they gate nothing
// here. A source that cannot answer must not take the identity envelope down
// with it, so the envelope goes out without them and the failure is logged.
async fn subject_attributes(
    state: &BridgeRouteState,
    user_id: &UserId,
) -> BTreeMap<String, Vec<String>> {
    match state.subjects.resolve_all(user_id).await {
        Ok(resolved) => resolved
            .iter()
            .map(|(dimension, values)| (dimension.as_str().to_owned(), values.to_vec()))
            .collect(),
        Err(SubjectSourceError::NotReady) => {
            tracing::debug!(%user_id, "subject attributes for whoami withheld: source not ready");
            BTreeMap::new()
        },
        Err(error) => {
            tracing::warn!(%error, %user_id, "subject attributes for whoami could not be read");
            BTreeMap::new()
        },
    }
}
