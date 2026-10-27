//! Gateway request authentication: one principal for every credential kind.
//!
//! A bridge JWT, an OAuth JWT and a personal access token (API key) all
//! resolve to the same [`Principal`]: the user, the roles loaded from the user
//! record (through the extractor's short-TTL user cache), the client, and —
//! for an API key only — the key's scopes, which narrow that principal rather
//! than standing in for it. Groups, projects and every other registered subject
//! dimension are loaded once per request through the same
//! [`SubjectProviderSet`] lookup for every credential kind, and scope
//! attribution reads them from the principal instead of asking again.
//!
//! Session ids are bound to the credential (see [`super::session_binding`]): a
//! JWT carries its session in the claim, and an API key gets a session minted
//! and bound to it. An `x-session-id` that differs from the bound session is
//! recorded as the audit row's `client_session_id` and never rejected: the
//! session always comes from the credential, so a client-sent id is audit
//! data, never an authorization input.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::{HeaderMap, StatusCode};
use std::collections::BTreeMap;
use std::sync::Arc;
use systemprompt_governance::authz::{SubjectAttributes, SubjectProviderSet};
use systemprompt_governance::policy::types::AccessScope;
use systemprompt_identifiers::{
    Actor, ApiKeyId, ClientId, ClientSessionId, JwtToken, SessionId, TraceId, UserId,
};
use systemprompt_models::attribution::ScopeBinding;
use systemprompt_models::execution::ContextExtractionError;
use systemprompt_runtime::AppContext;
use systemprompt_security::JwtUserContext;
use systemprompt_users::{API_KEY_PREFIX, ApiKeyLimits, ApiKeyService};

use super::error::RejectionError;
use super::session_binding::{SessionRequest, bind_api_key_session};
use crate::services::middleware::JwtContextExtractor;

/// What an API key narrows its user's principal to.
#[derive(Debug, Clone)]
pub struct KeyScopes {
    pub api_key_id: ApiKeyId,
    pub limits: ApiKeyLimits,
    pub scopes: Vec<ScopeBinding>,
}

/// The identity every credential kind resolves to.
#[derive(Debug, Clone)]
pub struct Principal {
    pub user_id: UserId,
    pub roles: Vec<String>,
    pub subject: SubjectAttributes,
    // JSON: ABAC attribute bag — JWT claim values are policy-defined and schema-less.
    pub attributes: BTreeMap<String, serde_json::Value>,
    pub act_chain: Vec<Actor>,
    pub client_id: Option<ClientId>,
    pub scopes: Option<KeyScopes>,
}

/// An authenticated gateway caller: the principal plus the session the
/// request is bound to.
#[derive(Debug, Clone)]
pub struct AuthedPrincipal {
    pub principal: Principal,
    pub trace_id: TraceId,
    pub session_id: SessionId,
    pub client_session: Option<ClientSessionId>,
}

impl AuthedPrincipal {
    pub const fn user_id(&self) -> &UserId {
        &self.principal.user_id
    }

    pub const fn trace_id(&self) -> &TraceId {
        &self.trace_id
    }

    pub const fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    pub const fn client_session(&self) -> Option<&ClientSessionId> {
        self.client_session.as_ref()
    }

    pub fn access_scope(&self) -> AccessScope {
        AccessScope::from_roles(&self.principal.roles)
    }

    // JSON: ABAC attribute bag — JWT claim values are policy-defined and
    // schema-less.
    pub fn authz_attributes(
        &self,
    ) -> (Vec<String>, BTreeMap<String, serde_json::Value>, Vec<Actor>) {
        (
            self.principal.roles.clone(),
            self.principal.attributes.clone(),
            self.principal.act_chain.clone(),
        )
    }

    pub const fn client_id(&self) -> Option<&ClientId> {
        self.principal.client_id.as_ref()
    }

    pub const fn api_key(&self) -> Option<&KeyScopes> {
        self.principal.scopes.as_ref()
    }

    pub fn is_bridge(&self) -> bool {
        self.principal.client_id.as_ref() == Some(&ClientId::bridge())
    }
}

/// What [`authenticate`] needs besides the credential itself.
#[derive(Clone, Copy)]
pub struct AuthInputs<'a> {
    pub header_session: Option<&'a SessionId>,
    pub headers: &'a HeaderMap,
    pub jwt_extractor: &'a JwtContextExtractor,
    pub providers: &'a SubjectProviderSet,
    pub ctx: &'a AppContext,
}

impl std::fmt::Debug for AuthInputs<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthInputs")
            .field("header_session", &self.header_session)
            .finish_non_exhaustive()
    }
}

pub async fn authenticate(
    credential: &str,
    inputs: AuthInputs<'_>,
) -> Result<AuthedPrincipal, RejectionError> {
    let mut authed = if credential.starts_with(API_KEY_PREFIX) {
        authenticate_api_key(credential, inputs).await?
    } else {
        authenticate_jwt(credential, inputs).await?
    };
    authed.principal.subject = inputs
        .providers
        .gather(&authed.principal.user_id)
        .await
        .map_err(|error| {
            RejectionError::server(
                StatusCode::INTERNAL_SERVER_ERROR,
                "subject attribute lookup failed",
            )
            .with_cause(error)
        })?;
    Ok(authed)
}

async fn authenticate_api_key(
    credential: &str,
    inputs: AuthInputs<'_>,
) -> Result<AuthedPrincipal, RejectionError> {
    let ctx = inputs.ctx;
    let service = ApiKeyService::new(
        Arc::clone(ctx.user_repository()),
        systemprompt_users::ApiKeyLifetime::from_max_days(ctx.config().api_key_max_lifetime_days),
    );
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

    let user = inputs
        .jwt_extractor
        .load_user(&rec.user_id, "gateway")
        .await
        .map_err(rejection_for_token)?;

    let binding = bind_api_key_session(
        ctx,
        SessionRequest {
            api_key_id: &rec.id,
            user_id: &rec.user_id,
            header_session: inputs.header_session,
            headers: inputs.headers,
        },
    )
    .await?;

    Ok(AuthedPrincipal {
        principal: Principal {
            user_id: rec.user_id,
            roles: user.roles,
            subject: SubjectAttributes::new(),
            attributes: BTreeMap::new(),
            act_chain: Vec::new(),
            client_id: None,
            scopes: Some(KeyScopes {
                api_key_id: rec.id,
                limits: rec.limits,
                scopes: rec.scopes,
            }),
        },
        trace_id: TraceId::generate(),
        session_id: binding.session_id,
        client_session: binding.client_session,
    })
}

async fn authenticate_jwt(
    credential: &str,
    inputs: AuthInputs<'_>,
) -> Result<AuthedPrincipal, RejectionError> {
    let jwt_token = JwtToken::new(credential);
    let (claims, user) = inputs
        .jwt_extractor
        .decode_for_gateway(&jwt_token)
        .await
        .map_err(rejection_for_token)?;

    let client_session = inputs
        .header_session
        .filter(|header| header.as_str() != claims.session_id.as_str())
        .and_then(|header| ClientSessionId::try_new(header.as_str().to_owned()).ok());

    Ok(jwt_principal(claims, user.roles, client_session))
}

fn jwt_principal(
    claims: JwtUserContext,
    roles: Vec<String>,
    client_session: Option<ClientSessionId>,
) -> AuthedPrincipal {
    AuthedPrincipal {
        principal: Principal {
            user_id: claims.user_id,
            roles,
            subject: SubjectAttributes::new(),
            attributes: claims.attributes,
            act_chain: claims.act_chain,
            client_id: claims.client_id,
            scopes: None,
        },
        trace_id: TraceId::generate(),
        session_id: claims.session_id,
        client_session,
    }
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
