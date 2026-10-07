//! Gateway request authentication: JWT session binding and API keys.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::StatusCode;
use std::collections::BTreeMap;
use std::sync::Arc;
use systemprompt_identifiers::{Actor, ApiKeyId, ClientId, JwtToken, SessionId, TraceId, UserId};
use systemprompt_models::execution::ContextExtractionError;
use systemprompt_runtime::AppContext;
use systemprompt_security::policy::types::AccessScope;
use systemprompt_users::{API_KEY_PREFIX, ApiKeyService};

use super::error::RejectionError;
use crate::services::middleware::JwtContextExtractor;
use crate::services::middleware::session::{SessionAttestationError, attest_session};
use systemprompt_traits::AppContext as _;

const UNKNOWN_SESSION_MESSAGE: &str =
    "unknown or revoked session; mint one at POST /api/public/gateway/sessions";

#[derive(Debug)]
pub enum AuthedPrincipal {
    Jwt(JwtPrincipal),
    ApiKey(ApiKeyPrincipal),
}

#[derive(Debug)]
pub struct JwtPrincipal {
    pub user_id: UserId,
    pub trace_id: TraceId,
    pub roles: Vec<String>,
    // JSON: ABAC attribute bag — JWT claim values are policy-defined and schema-less.
    pub attributes: BTreeMap<String, serde_json::Value>,
    pub act_chain: Vec<Actor>,
    pub attested_session: SessionId,
    pub client_id: Option<ClientId>,
}

#[derive(Debug)]
pub struct ApiKeyPrincipal {
    pub api_key_id: ApiKeyId,
    pub user_id: UserId,
    pub trace_id: TraceId,
    pub attested_session: SessionId,
}

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

    pub fn access_scope(&self) -> AccessScope {
        match self {
            Self::Jwt(p) => AccessScope::from_roles(&p.roles),
            Self::ApiKey(_) => AccessScope::Unknown,
        }
    }

    // JSON: ABAC attribute bag — JWT claim values are policy-defined and
    // schema-less.
    pub fn authz_attributes(
        &self,
    ) -> (Vec<String>, BTreeMap<String, serde_json::Value>, Vec<Actor>) {
        match self {
            Self::Jwt(p) => (p.roles.clone(), p.attributes.clone(), p.act_chain.clone()),
            Self::ApiKey(_) => (Vec::new(), BTreeMap::new(), Vec::new()),
        }
    }

    pub const fn client_id(&self) -> Option<&ClientId> {
        match self {
            Self::Jwt(p) => p.client_id.as_ref(),
            Self::ApiKey(_) => None,
        }
    }

    pub const fn api_key(&self) -> Option<&ApiKeyPrincipal> {
        match self {
            Self::Jwt(_) => None,
            Self::ApiKey(p) => Some(p),
        }
    }

    pub fn is_bridge(&self) -> bool {
        match self {
            Self::Jwt(p) => p.client_id.as_ref() == Some(&ClientId::bridge()),
            Self::ApiKey(_) => false,
        }
    }

    pub fn enforce_session_binding(&self, header: &SessionId) -> Result<(), RejectionError> {
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
        Err(RejectionError::client(
            StatusCode::UNAUTHORIZED,
            "X-Session-ID does not match authenticated session",
        ))
    }
}

pub async fn authenticate(
    credential: &str,
    session_id: &SessionId,
    jwt_extractor: &JwtContextExtractor,
    ctx: &AppContext,
) -> Result<AuthedPrincipal, RejectionError> {
    if credential.starts_with(API_KEY_PREFIX) {
        return authenticate_api_key(credential, session_id, ctx).await;
    }
    authenticate_jwt(credential, jwt_extractor).await
}

async fn authenticate_api_key(
    credential: &str,
    session_id: &SessionId,
    ctx: &AppContext,
) -> Result<AuthedPrincipal, RejectionError> {
    let service = ApiKeyService::new(Arc::clone(ctx.user_repository()));
    let record = service.verify(credential).await.map_err(|e| {
        RejectionError::server(
            StatusCode::INTERNAL_SERVER_ERROR,
            "API key verification failed",
        )
        .with_cause(e)
    })?;
    let Some(rec) = record else {
        return Err(RejectionError::client(
            StatusCode::UNAUTHORIZED,
            "Invalid or revoked API key",
        ));
    };

    let analytics = ctx.session_provider().ok_or_else(|| {
        RejectionError::server(
            StatusCode::INTERNAL_SERVER_ERROR,
            "analytics provider unavailable: cannot attest session",
        )
    })?;

    attest_session(&analytics, session_id, &rec.user_id, "gateway/messages")
        .await
        .map_err(|e| match e {
            SessionAttestationError::Missing | SessionAttestationError::UserMismatch => {
                RejectionError::client(StatusCode::UNAUTHORIZED, UNKNOWN_SESSION_MESSAGE)
            },
            lookup @ SessionAttestationError::Lookup(..) => RejectionError::server(
                StatusCode::INTERNAL_SERVER_ERROR,
                "session attestation failed",
            )
            .with_cause(lookup),
        })?;

    Ok(AuthedPrincipal::ApiKey(ApiKeyPrincipal {
        api_key_id: rec.id,
        user_id: rec.user_id,
        trace_id: TraceId::generate(),
        attested_session: session_id.clone(),
    }))
}

async fn authenticate_jwt(
    credential: &str,
    jwt_extractor: &JwtContextExtractor,
) -> Result<AuthedPrincipal, RejectionError> {
    let jwt_token = JwtToken::new(credential);
    let (claims, user) = jwt_extractor
        .decode_for_gateway(&jwt_token)
        .await
        .map_err(rejection_for_token)?;

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

fn rejection_for_token(error: ContextExtractionError) -> RejectionError {
    match error {
        ContextExtractionError::DatabaseError { .. } => RejectionError::server(
            StatusCode::INTERNAL_SERVER_ERROR,
            "gateway credential lookup failed",
        )
        .with_cause(error),
        other => RejectionError::invalid(StatusCode::UNAUTHORIZED, other),
    }
}
