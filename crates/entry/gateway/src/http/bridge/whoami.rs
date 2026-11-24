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

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::Json;
use axum::http::HeaderMap;
use systemprompt_identifiers::UserId;
use systemprompt_models::authz::{SubjectAttributeSource, SubjectSourceError};
use systemprompt_models::bridge::gateway::WhoamiResponse;
use systemprompt_runtime::AppContext;

use super::bridge_error::{BridgeError, authenticate_bridge};
use crate::kernel::{ApiHttpError, JwtContextExtractor};

pub async fn handle(
    jwt_extractor: Arc<JwtContextExtractor>,
    ctx: AppContext,
    headers: HeaderMap,
) -> Result<Json<WhoamiResponse>, ApiHttpError> {
    let (claims, _user) = authenticate_bridge(&jwt_extractor, &headers).await?;

    let user = ctx
        .user_repository()
        .find_by_id(&claims.user_id)
        .await
        .map_err(BridgeError::from)?
        .ok_or_else(|| BridgeError::UserNotFound(claims.user_id.clone()))?;

    let attributes = subject_attributes(&ctx, &claims.user_id).await;

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
async fn subject_attributes(ctx: &AppContext, user_id: &UserId) -> BTreeMap<String, Vec<String>> {
    match ctx.subject_handle().resolve_all(user_id).await {
        Ok(resolved) => resolved
            .iter()
            .map(|(dimension, values)| (dimension.as_str().to_owned(), values.to_vec()))
            .collect(),
        Err(SubjectSourceError::NotReady) => BTreeMap::new(),
        Err(error) => {
            tracing::warn!(%error, %user_id, "subject attributes for whoami could not be read");
            BTreeMap::new()
        },
    }
}
