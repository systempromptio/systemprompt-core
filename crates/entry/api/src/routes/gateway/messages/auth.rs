use axum::http::StatusCode;
use std::collections::BTreeMap;
use systemprompt_identifiers::{Actor, JwtToken, SessionId, TraceId, UserId};
use systemprompt_runtime::AppContext;
use systemprompt_users::{API_KEY_PREFIX, ApiKeyService};

use crate::services::middleware::JwtContextExtractor;

pub(super) enum AuthedPrincipal {
    Jwt(JwtPrincipal),
    ApiKey(ApiKeyPrincipal),
}

pub(super) struct JwtPrincipal {
    pub user_id: UserId,
    pub trace_id: TraceId,
    pub roles: Vec<String>,
    pub attributes: BTreeMap<String, serde_json::Value>,
    pub act_chain: Vec<Actor>,
    attested_session: SessionId,
}

pub(super) struct ApiKeyPrincipal {
    pub user_id: UserId,
    pub trace_id: TraceId,
}

impl AuthedPrincipal {
    pub(super) fn user_id(&self) -> &UserId {
        match self {
            Self::Jwt(p) => &p.user_id,
            Self::ApiKey(p) => &p.user_id,
        }
    }

    pub(super) fn trace_id(&self) -> &TraceId {
        match self {
            Self::Jwt(p) => &p.trace_id,
            Self::ApiKey(p) => &p.trace_id,
        }
    }

    pub(super) fn authz_attributes(
        &self,
    ) -> (Vec<String>, BTreeMap<String, serde_json::Value>, Vec<Actor>) {
        match self {
            Self::Jwt(p) => (
                p.roles.clone(),
                p.attributes.clone(),
                p.act_chain.clone(),
            ),
            Self::ApiKey(_) => (Vec::new(), BTreeMap::new(), Vec::new()),
        }
    }

    pub(super) fn enforce_session_binding(
        &self,
        header: &SessionId,
    ) -> Result<(), (StatusCode, String)> {
        match self {
            Self::Jwt(p) => p.enforce_session_binding(header),
            Self::ApiKey(_) => Ok(()),
        }
    }
}

impl JwtPrincipal {
    fn enforce_session_binding(&self, header: &SessionId) -> Result<(), (StatusCode, String)> {
        if self.attested_session.as_str() == header.as_str() {
            return Ok(());
        }
        tracing::warn!(
            header_session = %header.as_str(),
            jwt_session = %self.attested_session.as_str(),
            user_id = %self.user_id,
            "X-Session-ID header does not match bearer JWT session_id; rejecting"
        );
        Err((
            StatusCode::UNAUTHORIZED,
            "X-Session-ID does not match authenticated session".to_owned(),
        ))
    }
}

pub(super) async fn authenticate(
    credential: &str,
    jwt_extractor: &JwtContextExtractor,
    ctx: &AppContext,
) -> Result<AuthedPrincipal, (StatusCode, String)> {
    if credential.starts_with(API_KEY_PREFIX) {
        return authenticate_api_key(credential, ctx).await;
    }
    authenticate_jwt(credential, jwt_extractor, ctx).await
}

async fn authenticate_api_key(
    credential: &str,
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
    match record {
        Some(rec) => Ok(AuthedPrincipal::ApiKey(ApiKeyPrincipal {
            user_id: rec.user_id,
            trace_id: TraceId::generate(),
        })),
        None => Err((
            StatusCode::UNAUTHORIZED,
            "Invalid or revoked API key".to_owned(),
        )),
    }
}

async fn authenticate_jwt(
    credential: &str,
    jwt_extractor: &JwtContextExtractor,
    ctx: &AppContext,
) -> Result<AuthedPrincipal, (StatusCode, String)> {
    let jwt_token = JwtToken::new(credential);
    let claims = jwt_extractor
        .decode_for_gateway(&jwt_token)
        .await
        .map_err(|e| (StatusCode::UNAUTHORIZED, e.to_string()))?;

    let repo = systemprompt_users::UserRepository::new(ctx.db_pool())
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let user = repo
        .find_by_id(&claims.user_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                format!("User not found: {}", claims.user_id.as_str()),
            )
        })?;

    Ok(AuthedPrincipal::Jwt(JwtPrincipal {
        user_id: claims.user_id,
        trace_id: TraceId::generate(),
        roles: user.roles,
        attributes: claims.attributes,
        act_chain: claims.act_chain,
        attested_session: claims.session_id,
    }))
}
