//! RFC 8693 OAuth 2.0 Token Exchange.
//!
//! Trades a `subject_token` issued by a trusted federated identity provider
//! (or by this deployment itself) for a delegated access token bound to the
//! authenticated client. The resulting token carries an `act` claim chain
//! that records every actor who participated in the delegation, oldest
//! delegator innermost. The endpoint also enforces:
//!
//! * `subject_token` issuer is in `profile.security.trusted_issuers` (or is our
//!   own deployment) and signature verifies against that issuer's JWKS;
//! * subject audience matches the trusted-issuer record;
//! * requested `scope` is at most the intersection of subject scope, client
//!   scope, and owner permissions;
//! * `resource` (RFC 8707) is in `allowed_resource_audiences`, otherwise the
//!   call is rejected with `invalid_target`.

use std::str::FromStr;

use anyhow::{Result, anyhow};
use axum::http::HeaderMap;
use systemprompt_identifiers::{ClientId, SessionId, UserId};
use systemprompt_models::Config;
use systemprompt_models::auth::{AuthenticatedUser, Permission, parse_permissions};
use systemprompt_oauth::OAuthState;
use systemprompt_oauth::repository::OAuthRepository;
use systemprompt_oauth::services::{JwtConfig, JwtSigningParams, generate_jwt_with_act};

use super::super::{TokenError, TokenResponse};

mod claims;
mod subject;

pub use claims::{build_act_chain, intersect_scopes};
pub use subject::peek_issuer;

use claims::resolve_audience;
use subject::validate_subject_token;

pub const ACCESS_TOKEN_TYPE: &str = "urn:ietf:params:oauth:token-type:access_token";
pub const ID_TOKEN_TYPE: &str = "urn:ietf:params:oauth:token-type:id_token";
pub const JWT_TOKEN_TYPE: &str = "urn:ietf:params:oauth:token-type:jwt";

#[derive(Debug, Default)]
pub struct TokenExchangeRequest<'a> {
    pub subject_token: &'a str,
    pub subject_token_type: &'a str,
    pub actor_token: Option<&'a str>,
    pub actor_token_type: Option<&'a str>,
    pub requested_token_type: Option<&'a str>,
    pub scope: Option<&'a str>,
    pub audience: Option<&'a str>,
    pub resource: Option<&'a str>,
}

pub async fn handle_token_exchange(
    repo: &OAuthRepository,
    client_id: &ClientId,
    request: TokenExchangeRequest<'_>,
    headers: &HeaderMap,
    state: &OAuthState,
) -> Result<TokenResponse> {
    let global = Config::get()?;

    let subject =
        validate_subject_token(request.subject_token, request.subject_token_type, global).await?;

    let resource = match request.resource {
        Some(value)
            if !global
                .allowed_resource_audiences
                .iter()
                .any(|allowed| allowed == value) =>
        {
            return Err(anyhow!(TokenError::InvalidTarget {
                message: format!("'{value}' not in allowed_resource_audiences"),
            }));
        },
        other => other,
    };

    let client = repo
        .find_client_by_id(client_id)
        .await?
        .ok_or_else(|| anyhow!(TokenError::InvalidClient))?;
    let owner = state
        .user_provider()
        .find_by_id(&client.owner_user_id)
        .await
        .map_err(|e| anyhow!("Failed to load client owner: {e}"))?
        .ok_or_else(|| anyhow!("Client owner not found"))?;
    if !owner.is_active {
        return Err(anyhow!("Client owner is not active"));
    }
    let owner_perms: Vec<Permission> = owner
        .roles
        .iter()
        .filter_map(|r| Permission::from_str(r).ok())
        .collect();
    let client_perms: Vec<Permission> = client
        .scopes
        .iter()
        .filter_map(|s| Permission::from_str(s).ok())
        .collect();
    let requested_perms = match request.scope {
        Some(s) => parse_permissions(s)?,
        None => subject.scope.clone(),
    };

    let final_perms = intersect_scopes(
        &requested_perms,
        &subject.scope,
        &client_perms,
        &owner_perms,
    )?;

    let audience = resolve_audience(request.audience, resource, global)?;

    let issuer = &global.jwt_issuer;
    let act = build_act_chain(client_id, issuer, subject.prior_act);

    let owner_uuid = uuid::Uuid::parse_str(client.owner_user_id.as_str())
        .map_err(|e| anyhow!("Client owner has a non-uuid id ({e})"))?;
    let role_strings: Vec<String> = final_perms.iter().map(ToString::to_string).collect();
    let delegated_user = AuthenticatedUser::new_with_roles(
        owner_uuid,
        owner.name.clone(),
        owner.email.clone(),
        final_perms.clone(),
        role_strings,
    );

    let session_id = ensure_session(state, headers, &client.owner_user_id, global).await?;

    let config = JwtConfig {
        permissions: final_perms.clone(),
        audience: audience.clone(),
        expires_in_hours: Some(global.jwt_access_token_expiration / 3600),
        resource: resource.map(str::to_string),
        plugin_id: None,
    };
    let signing = JwtSigningParams {
        issuer: &global.jwt_issuer,
    };

    let access_token = generate_jwt_with_act(
        &delegated_user,
        config,
        uuid::Uuid::new_v4().to_string(),
        &session_id,
        &signing,
        act,
    )?;

    let scope_string = final_perms
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(" ");

    Ok(TokenResponse {
        access_token,
        token_type: "Bearer".to_string(),
        expires_in: global.jwt_access_token_expiration,
        refresh_token: None,
        scope: Some(scope_string),
        issued_token_type: Some(ACCESS_TOKEN_TYPE.to_string()),
    })
}

async fn ensure_session(
    state: &OAuthState,
    headers: &HeaderMap,
    owner_user_id: &UserId,
    global: &Config,
) -> Result<SessionId> {
    use systemprompt_identifiers::SessionSource;
    use systemprompt_traits::CreateSessionInput;

    let session_id = SessionId::new(format!("sess_{}", uuid::Uuid::new_v4().simple()));
    let expires_at =
        chrono::Utc::now() + chrono::Duration::seconds(global.jwt_access_token_expiration);
    let analytics = state.analytics_provider().extract_analytics(headers, None);
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
        .map_err(|e| anyhow!("Failed to create session: {e}"))?;
    Ok(session_id)
}
