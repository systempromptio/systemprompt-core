//! Gateway request authentication: JWT session binding and API keys.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::StatusCode;
use std::collections::BTreeMap;
use std::sync::Arc;
use systemprompt_identifiers::{Actor, ClientId, JwtToken, SessionId, TraceId, UserId};
use systemprompt_runtime::AppContext;
use systemprompt_security::policy::types::AccessScope;
use systemprompt_users::{API_KEY_PREFIX, ApiKeyService};

use crate::services::middleware::JwtContextExtractor;
use crate::services::middleware::session::{SessionAttestationError, attest_session};
use systemprompt_traits::AppContext as _;

const UNKNOWN_SESSION_MESSAGE: &str =
    "unknown or revoked session; mint one at POST /api/public/gateway/sessions";

#[derive(Debug)]
pub enum AuthedPrincipal {
    Jwt(JwtPrincipal),
    ApiKey(ApiKeyPrincipal),
    Execution(ExecutionPrincipal),
}

#[derive(Debug)]
pub struct ExecutionPrincipal {
    pub principal: systemprompt_evaluation::repository::experiments::ExecutionPrincipal,
    pub trace_id: TraceId,
}

#[derive(Debug)]
pub struct JwtPrincipal {
    pub user_id: UserId,
    pub trace_id: TraceId,
    pub roles: Vec<String>,
    pub attributes: BTreeMap<String, serde_json::Value>,
    pub act_chain: Vec<Actor>,
    pub attested_session: SessionId,
    pub client_id: Option<ClientId>,
}

#[derive(Debug)]
pub struct ApiKeyPrincipal {
    pub user_id: UserId,
    pub trace_id: TraceId,
    pub attested_session: SessionId,
}

impl AuthedPrincipal {
    pub const fn user_id(&self) -> &UserId {
        match self {
            Self::Jwt(p) => &p.user_id,
            Self::ApiKey(p) => &p.user_id,
            Self::Execution(p) => &p.principal.identity.owner_id,
        }
    }

    pub const fn trace_id(&self) -> &TraceId {
        match self {
            Self::Jwt(p) => &p.trace_id,
            Self::ApiKey(p) => &p.trace_id,
            Self::Execution(p) => &p.trace_id,
        }
    }

    pub const fn attested_session(&self) -> &SessionId {
        match self {
            Self::Jwt(p) => &p.attested_session,
            Self::ApiKey(p) => &p.attested_session,
            Self::Execution(p) => &p.principal.session_id,
        }
    }

    pub fn access_scope(&self) -> AccessScope {
        match self {
            Self::Jwt(p) => AccessScope::from_roles(&p.roles),
            Self::ApiKey(_) => AccessScope::Unknown,
            Self::Execution(p) => AccessScope::from_roles(&p.principal.identity.roles),
        }
    }

    pub fn authz_attributes(
        &self,
    ) -> (Vec<String>, BTreeMap<String, serde_json::Value>, Vec<Actor>) {
        match self {
            Self::Jwt(p) => (p.roles.clone(), p.attributes.clone(), p.act_chain.clone()),
            Self::ApiKey(_) => (Vec::new(), BTreeMap::new(), Vec::new()),
            Self::Execution(p) => (
                p.principal.identity.roles.clone(),
                BTreeMap::new(),
                vec![Actor::job(
                    p.principal.identity.owner_id.clone(),
                    format!("evaluation:{}", p.principal.identity.execution_id),
                )],
            ),
        }
    }

    pub const fn client_id(&self) -> Option<&ClientId> {
        match self {
            Self::Jwt(p) => p.client_id.as_ref(),
            Self::ApiKey(_) | Self::Execution(_) => None,
        }
    }

    pub fn enforce_session_binding(&self, header: &SessionId) -> Result<(), (StatusCode, String)> {
        let (attested, credential) = match self {
            Self::Jwt(p) => (&p.attested_session, "bearer JWT session_id"),
            Self::ApiKey(p) => (&p.attested_session, "attested API-key session"),
            Self::Execution(p) => (&p.principal.session_id, "execution capability session"),
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

pub async fn authenticate(
    credential: &str,
    session_id: &SessionId,
    jwt_extractor: &JwtContextExtractor,
    ctx: &AppContext,
    capabilities: &systemprompt_evaluation::repository::experiments::ExecutionCapabilityRepository,
) -> Result<AuthedPrincipal, (StatusCode, String)> {
    if credential
        .starts_with(systemprompt_evaluation::repository::experiments::EXECUTION_TOKEN_PREFIX)
    {
        let principal = capabilities
            .authenticate(credential, &ctx.config().api_external_url)
            .await
            .map_err(execution_auth_error)?;
        return Ok(AuthedPrincipal::Execution(ExecutionPrincipal {
            principal,
            trace_id: TraceId::generate(),
        }));
    }
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
    let service = ApiKeyService::new(Arc::clone(ctx.user_repository()));
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
        client_id: claims.client_id,
    }))
}

fn execution_auth_error(error: systemprompt_evaluation::EvaluationError) -> (StatusCode, String) {
    match error {
        systemprompt_evaluation::EvaluationError::ResourceNotFound(_) => (
            StatusCode::UNAUTHORIZED,
            "Invalid or expired execution capability".to_owned(),
        ),
        other => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Execution authentication failed: {other}"),
        ),
    }
}
