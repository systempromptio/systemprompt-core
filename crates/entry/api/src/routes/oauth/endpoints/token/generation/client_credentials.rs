//! `client_credentials` grant token generation (RFC 6749 §4.4).
//!
//! Mints an access token for a client acting as itself, intersecting the
//! requested scopes with both the client's static grant and (for delegated
//! user-tier roles) the owner's permissions. [`ClientCredentialsError`]
//! partitions failures so the route maps recoverable client mistakes to 4xx.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::str::FromStr;
use systemprompt_identifiers::{ClientId, SessionId, SessionSource, UserId};
use systemprompt_models::Config;
use systemprompt_models::auth::{
    AuthenticatedUser, JwtAudience, Permission, parse_permissions, permissions_to_string,
};
use systemprompt_oauth::OAuthState;
use systemprompt_oauth::repository::OAuthRepository;
use systemprompt_oauth::services::{JwtConfig, JwtSigningParams, generate_jwt};
use systemprompt_traits::{CreateSessionInput, ExtractSignals};
use thiserror::Error;

use super::super::TokenResponse;
use super::RequestOrigin;

#[derive(Debug, Default)]
pub struct ClientTokenOptions<'a> {
    pub scope: Option<&'a str>,
    pub plugin_id: Option<&'a str>,
    pub audience: Option<&'a str>,
}

/// Failure modes of the `client_credentials` grant.
///
/// Variants partition by RFC 6749 §5.2 error code so the route handler can map
/// each to the right HTTP status. Recoverable client mistakes (unknown client,
/// orphaned or inactive owner, bad scope/audience) must surface as 4xx, never
/// 5xx — the latter masks operator-visible misconfiguration as gateway
/// failures and triggers spurious paging.
#[derive(Debug, Error)]
pub enum ClientCredentialsError {
    #[error("Client not found")]
    ClientNotFound,
    #[error("Client owner not found")]
    OwnerNotFound,
    #[error("Client owner is not active")]
    OwnerInactive,
    #[error("Client owner has a non-uuid id ({0})")]
    OwnerIdMalformed(String),
    #[error("Invalid scope: {0}")]
    InvalidScope(String),
    #[error("Invalid audience: {0}")]
    InvalidAudience(String),
    #[error("Hook scopes require audience=hook on the token request")]
    HookScopeRequiresHookAudience,
    #[error("Failed to load client owner: {0}")]
    UserProviderUnavailable(#[source] Box<dyn std::error::Error + Send + Sync>),
    #[error("Failed to create session: {0}")]
    SessionCreate(#[source] Box<dyn std::error::Error + Send + Sync>),
    #[error("JWT signing failed: {0}")]
    JwtSign(#[source] Box<dyn std::error::Error + Send + Sync>),
    #[error("Config unavailable: {0}")]
    ConfigUnavailable(#[source] Box<dyn std::error::Error + Send + Sync>),
}

struct OwnerProfile {
    name: String,
    email: String,
    permissions: Vec<Permission>,
}

async fn load_active_owner(
    state: &OAuthState,
    owner_user_id: &UserId,
) -> Result<OwnerProfile, ClientCredentialsError> {
    let owner = state
        .user_provider()
        .find_by_id(owner_user_id)
        .await
        .map_err(|e| ClientCredentialsError::UserProviderUnavailable(e.into()))?
        .ok_or(ClientCredentialsError::OwnerNotFound)?;
    if !owner.is_active {
        return Err(ClientCredentialsError::OwnerInactive);
    }
    Ok(OwnerProfile {
        permissions: scope_permissions(&owner.roles),
        name: owner.name,
        email: owner.email,
    })
}

async fn create_client_session(
    state: &OAuthState,
    origin: RequestOrigin<'_>,
    owner_user_id: &UserId,
    expires_in: i64,
) -> Result<SessionId, ClientCredentialsError> {
    let session_id = SessionId::new(format!("sess_{}", uuid::Uuid::new_v4().simple()));
    let expires_at = chrono::Utc::now() + chrono::Duration::seconds(expires_in);
    let analytics = state.analytics_provider().extract_analytics(
        origin.headers,
        ExtractSignals {
            caller_ip: origin.caller_ip,
            ..Default::default()
        },
    );

    state
        .analytics_provider()
        .create_session(CreateSessionInput {
            session_id: &session_id,
            user_id: Some(owner_user_id),
            analytics: &analytics,
            session_source: SessionSource::Oauth,
            is_bot: false,
            is_ai_crawler: false,
            expires_at,
        })
        .await
        .map_err(|e| ClientCredentialsError::SessionCreate(e.into()))?;
    Ok(session_id)
}

pub async fn generate_client_tokens(
    repo: &OAuthRepository,
    client_id: &ClientId,
    origin: RequestOrigin<'_>,
    state: &OAuthState,
    options: ClientTokenOptions<'_>,
) -> Result<TokenResponse, ClientCredentialsError> {
    let global_config =
        Config::get().map_err(|e| ClientCredentialsError::ConfigUnavailable(e.into()))?;
    let expires_in = global_config.jwt_access_token_expiration;

    let client = repo
        .find_client_by_id(client_id)
        .await
        .map_err(|e| ClientCredentialsError::UserProviderUnavailable(e.into()))?
        .ok_or(ClientCredentialsError::ClientNotFound)?;

    let requested_permissions = match options.scope {
        Some(scope_str) => parse_permissions(scope_str)
            .map_err(|e| ClientCredentialsError::InvalidScope(e.to_string()))?,
        None => scope_permissions(&client.scopes),
    };

    let owner = load_active_owner(state, &client.owner_user_id).await?;

    let permissions =
        authorize_client_grant(&requested_permissions, &client.scopes, &owner.permissions)?;

    let audience = resolve_audience(options.audience, global_config)?;

    if permissions.iter().any(Permission::is_hook_scope)
        && !audience.iter().any(|a| matches!(a, JwtAudience::Hook))
    {
        return Err(ClientCredentialsError::HookScopeRequiresHookAudience);
    }

    let owner_uuid = uuid::Uuid::parse_str(client.owner_user_id.as_str())
        .map_err(|e| ClientCredentialsError::OwnerIdMalformed(e.to_string()))?;
    let authenticated =
        AuthenticatedUser::new(owner_uuid, owner.name, owner.email, permissions.clone());

    let config = JwtConfig {
        permissions: permissions.clone(),
        audience,
        expires_in_hours: Some(global_config.jwt_access_token_expiration / 3600),
        plugin_id: options.plugin_id.map(str::to_owned),
        ..Default::default()
    };
    let session_id =
        create_client_session(state, origin, &client.owner_user_id, expires_in).await?;

    let signing = JwtSigningParams {
        issuer: &global_config.jwt_issuer,
    };
    let jwt_token = generate_jwt(
        &authenticated,
        config,
        uuid::Uuid::new_v4().to_string(),
        &session_id,
        &signing,
    )
    .map_err(|e| ClientCredentialsError::JwtSign(e.into()))?;

    Ok(TokenResponse {
        access_token: jwt_token,
        token_type: "Bearer".to_owned(),
        expires_in,
        refresh_token: None,
        scope: Some(
            permissions
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(" "),
        ),
        issued_token_type: None,
    })
}

fn scope_permissions(scopes: &[String]) -> Vec<Permission> {
    scopes
        .iter()
        .filter_map(|s| Permission::from_str(s).ok())
        .collect()
}

// Why: service-tier scopes ([`Permission::is_service_scope`]) need only the
// client grant, but user-tier roles are delegated authority and require both
// the client *and* its owner to hold the permission — the RFC 6749 §4.4
// owner is audit attribution, never authorization by itself.
fn authorize_client_grant(
    requested: &[Permission],
    client_scopes: &[String],
    owner_permissions: &[Permission],
) -> Result<Vec<Permission>, ClientCredentialsError> {
    let client_allowed = scope_permissions(client_scopes);

    let mut granted: Vec<Permission> = Vec::with_capacity(requested.len());
    let mut missing_from_client: Vec<Permission> = Vec::new();
    let mut missing_from_owner: Vec<Permission> = Vec::new();

    for &perm in requested {
        if !client_allowed.contains(&perm) {
            missing_from_client.push(perm);
            continue;
        }
        match perm {
            Permission::HookGovern
            | Permission::HookTrack
            | Permission::Service
            | Permission::A2a
            | Permission::Mcp => granted.push(perm),
            Permission::Admin | Permission::User | Permission::Anonymous => {
                if owner_permissions.contains(&perm) {
                    granted.push(perm);
                } else {
                    missing_from_owner.push(perm);
                }
            },
        }
    }

    granted.sort_by_key(|p| std::cmp::Reverse(p.hierarchy_level()));
    granted.dedup();

    if granted.is_empty() {
        let reason = if !missing_from_client.is_empty() {
            format!(
                "requested scopes not in client grant: {}",
                permissions_to_string(&missing_from_client)
            )
        } else if !missing_from_owner.is_empty() {
            format!(
                "delegated scopes not held by owner: {}",
                permissions_to_string(&missing_from_owner)
            )
        } else {
            "no scopes requested".to_owned()
        };
        return Err(ClientCredentialsError::InvalidScope(reason));
    }

    Ok(granted)
}

fn resolve_audience(
    requested: Option<&str>,
    global_config: &Config,
) -> Result<Vec<JwtAudience>, ClientCredentialsError> {
    let Some(value) = requested else {
        return Ok(global_config.jwt_audiences.clone());
    };

    if !global_config
        .allowed_resource_audiences
        .iter()
        .any(|allowed| allowed == value)
    {
        return Err(ClientCredentialsError::InvalidAudience(format!(
            "'{value}' not in allowed audiences"
        )));
    }

    JwtAudience::from_str(value)
        .map(|aud| vec![aud])
        .map_err(|e| ClientCredentialsError::InvalidAudience(format!("'{value}': {e}")))
}
