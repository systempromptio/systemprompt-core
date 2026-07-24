//! Gateway request authentication: JWT session binding and API keys.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::StatusCode;
use std::collections::BTreeMap;
use systemprompt_identifiers::{Actor, JwtToken, SessionId, TraceId, UserId};
use systemprompt_runtime::AppContext;
use systemprompt_users::{API_KEY_PREFIX, ApiKeyService};

use crate::services::middleware::JwtContextExtractor;
use crate::services::middleware::session::{SessionAttestationError, attest_session};
use systemprompt_traits::AppContext as _;

const UNKNOWN_SESSION_MESSAGE: &str =
    "unknown or revoked session; mint one at POST /api/public/gateway/sessions";

#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "items are re-exported via `test_api` only when the feature is on"
    )
)]
#[derive(Debug)]
pub enum AuthedPrincipal {
    Jwt(JwtPrincipal),
    ApiKey(ApiKeyPrincipal),
}

#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "items are re-exported via `test_api` only when the feature is on"
    )
)]
#[derive(Debug)]
pub struct JwtPrincipal {
    pub user_id: UserId,
    pub trace_id: TraceId,
    pub roles: Vec<String>,
    pub attributes: BTreeMap<String, serde_json::Value>,
    pub act_chain: Vec<Actor>,
    pub attested_session: SessionId,
}

#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "items are re-exported via `test_api` only when the feature is on"
    )
)]
#[derive(Debug)]
pub struct ApiKeyPrincipal {
    pub user_id: UserId,
    pub trace_id: TraceId,
    pub attested_session: SessionId,
}

#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "items are re-exported via `test_api` only when the feature is on"
    )
)]
impl AuthedPrincipal {
    pub const fn user_id(&self) -> &UserId {
        match self {
            Self::Jwt(p) => &p.user_id,
            Self::ApiKey(p) => &p.user_id,
        }
    }

    pub const fn trace_id(&self) -> &TraceId {
        match self {
            Self::Jwt(p) => &p.trace_id,
            Self::ApiKey(p) => &p.trace_id,
        }
    }

    pub const fn attested_session(&self) -> &SessionId {
        match self {
            Self::Jwt(p) => &p.attested_session,
            Self::ApiKey(p) => &p.attested_session,
        }
    }

    pub fn authz_attributes(
        &self,
    ) -> (Vec<String>, BTreeMap<String, serde_json::Value>, Vec<Actor>) {
        match self {
            Self::Jwt(p) => (p.roles.clone(), p.attributes.clone(), p.act_chain.clone()),
            Self::ApiKey(_) => (Vec::new(), BTreeMap::new(), Vec::new()),
        }
    }

    pub fn enforce_session_binding(&self, header: &SessionId) -> Result<(), (StatusCode, String)> {
        let (attested, credential) = match self {
            Self::Jwt(p) => (&p.attested_session, "bearer JWT session_id"),
            Self::ApiKey(p) => (&p.attested_session, "attested API-key session"),
        };
        if attested.as_str() == header.as_str() {
            return Ok(());
        }
        tracing::warn!(
            header_session = %header.as_str(),
            attested_session = %attested.as_str(),
            user_id = %self.user_id(),
            credential = %credential,
            "X-Session-ID header does not match the attested session; rejecting"
        );
        Err((
            StatusCode::UNAUTHORIZED,
            "X-Session-ID does not match authenticated session".to_owned(),
        ))
    }
}

#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "re-exported via `test_api` only when the feature is on"
    )
)]
pub async fn authenticate(
    credential: &str,
    session_id: &SessionId,
    jwt_extractor: &JwtContextExtractor,
    ctx: &AppContext,
) -> Result<AuthedPrincipal, (StatusCode, String)> {
    if credential.starts_with(API_KEY_PREFIX) {
        return authenticate_api_key(credential, session_id, ctx).await;
    }
    authenticate_jwt(credential, jwt_extractor).await
}

async fn authenticate_api_key(
    credential: &str,
    session_id: &SessionId,
    ctx: &AppContext,
) -> Result<AuthedPrincipal, (StatusCode, String)> {
    let service = ApiKeyService::new(ctx.db_pool()).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("API key service unavailable: {e}"),
        )
    })?;
    let record = service.verify(credential).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("API key verification failed: {e}"),
        )
    })?;
    let Some(rec) = record else {
        return Err((
            StatusCode::UNAUTHORIZED,
            "Invalid or revoked API key".to_owned(),
        ));
    };

    let analytics = ctx.analytics_provider().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Analytics provider unavailable: cannot attest session".to_owned(),
        )
    })?;

    attest_session(&analytics, session_id, &rec.user_id, "gateway/messages")
        .await
        .map_err(|e| match e {
            SessionAttestationError::Lookup(message) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Session attestation failed: {message}"),
            ),
            SessionAttestationError::Missing | SessionAttestationError::UserMismatch => {
                (StatusCode::UNAUTHORIZED, UNKNOWN_SESSION_MESSAGE.to_owned())
            },
        })?;

    Ok(AuthedPrincipal::ApiKey(ApiKeyPrincipal {
        user_id: rec.user_id,
        trace_id: TraceId::generate(),
        attested_session: session_id.clone(),
    }))
}

async fn authenticate_jwt(
    credential: &str,
    jwt_extractor: &JwtContextExtractor,
) -> Result<AuthedPrincipal, (StatusCode, String)> {
    let jwt_token = JwtToken::new(credential);
    let (claims, user) = jwt_extractor
        .decode_for_gateway(&jwt_token)
        .await
        .map_err(|e| (StatusCode::UNAUTHORIZED, e.to_string()))?;

    Ok(AuthedPrincipal::Jwt(JwtPrincipal {
        user_id: claims.user_id,
        trace_id: TraceId::generate(),
        roles: user.roles,
        attributes: claims.attributes,
        act_chain: claims.act_chain,
        attested_session: claims.session_id,
    }))
}
