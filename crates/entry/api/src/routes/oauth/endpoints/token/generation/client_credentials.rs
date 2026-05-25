use anyhow::{Result, anyhow};
use axum::http::HeaderMap;
use std::str::FromStr;
use systemprompt_identifiers::{ClientId, SessionId, SessionSource};
use systemprompt_models::Config;
use systemprompt_models::auth::{AuthenticatedUser, JwtAudience, Permission, parse_permissions};
use systemprompt_oauth::OAuthState;
use systemprompt_oauth::repository::OAuthRepository;
use systemprompt_oauth::services::{JwtConfig, JwtSigningParams, generate_jwt};
use systemprompt_traits::CreateSessionInput;

use super::super::TokenResponse;

#[derive(Debug, Default)]
pub struct ClientTokenOptions<'a> {
    pub scope: Option<&'a str>,
    pub plugin_id: Option<&'a str>,
    pub audience: Option<&'a str>,
}

pub async fn generate_client_tokens(
    repo: &OAuthRepository,
    client_id: &ClientId,
    headers: &HeaderMap,
    state: &OAuthState,
    options: ClientTokenOptions<'_>,
) -> Result<TokenResponse> {
    let expires_in = Config::get()?.jwt_access_token_expiration;

    let client = repo
        .find_client_by_id(client_id)
        .await?
        .ok_or_else(|| anyhow!("Client not found"))?;

    let requested_permissions = match options.scope {
        Some(scope_str) => parse_permissions(scope_str)?,
        None => client_scope_permissions(&client.scopes),
    };

    let owner = state
        .user_provider()
        .find_by_id(&client.owner_user_id)
        .await
        .map_err(|e| anyhow!("Failed to load client owner: {e}"))?
        .ok_or_else(|| anyhow!("Client owner not found"))?;
    if !owner.is_active {
        return Err(anyhow!("Client owner is not active"));
    }
    let owner_permissions: Vec<Permission> = owner
        .roles
        .iter()
        .filter_map(|r| Permission::from_str(r).ok())
        .collect();

    let permissions =
        intersect_permissions(&requested_permissions, &client.scopes, &owner_permissions)?;

    let global_config = Config::get()?;
    let audience = resolve_audience(options.audience, global_config)?;

    if permissions.iter().any(Permission::is_hook_scope)
        && !audience.iter().any(|a| matches!(a, JwtAudience::Hook))
    {
        return Err(anyhow!(
            "Hook scopes require audience=hook on the token request"
        ));
    }

    let owner_uuid = uuid::Uuid::parse_str(client.owner_user_id.as_str())
        .map_err(|e| anyhow!("Client owner has a non-uuid id ({e})"))?;
    let role_strings: Vec<String> = permissions.iter().map(ToString::to_string).collect();
    let authenticated = AuthenticatedUser::new_with_roles(
        owner_uuid,
        owner.name.clone(),
        owner.email.clone(),
        permissions.clone(),
        role_strings,
    );

    let config = JwtConfig {
        permissions: permissions.clone(),
        audience,
        expires_in_hours: Some(global_config.jwt_access_token_expiration / 3600),
        plugin_id: options.plugin_id.map(str::to_owned),
        ..Default::default()
    };
    let session_id = SessionId::new(format!("sess_{}", uuid::Uuid::new_v4().simple()));
    let expires_at = chrono::Utc::now() + chrono::Duration::seconds(expires_in);
    let analytics = state.analytics_provider().extract_analytics(headers, None);

    state
        .analytics_provider()
        .create_session(CreateSessionInput {
            session_id: &session_id,
            user_id: Some(&client.owner_user_id),
            analytics: &analytics,
            session_source: SessionSource::Oauth,
            is_bot: false,
            is_ai_crawler: false,
            expires_at,
        })
        .await
        .map_err(|e| anyhow!("Failed to create session: {e}"))?;

    let signing = JwtSigningParams {
        issuer: &global_config.jwt_issuer,
    };
    let jwt_token = generate_jwt(
        &authenticated,
        config,
        uuid::Uuid::new_v4().to_string(),
        &session_id,
        &signing,
    )?;

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

fn client_scope_permissions(client_scopes: &[String]) -> Vec<Permission> {
    client_scopes
        .iter()
        .filter_map(|s| Permission::from_str(s).ok())
        .collect()
}

fn intersect_permissions(
    requested: &[Permission],
    client_scopes: &[String],
    owner_permissions: &[Permission],
) -> Result<Vec<Permission>> {
    let client_allowed: Vec<Permission> = client_scope_permissions(client_scopes);

    let allowed: Vec<Permission> = requested
        .iter()
        .filter(|p| client_allowed.contains(p) && owner_permissions.contains(p))
        .copied()
        .collect();

    if allowed.is_empty() {
        return Err(anyhow!(
            "No valid permissions: scopes not allowed for both client and owner"
        ));
    }

    Ok(allowed)
}

fn resolve_audience(requested: Option<&str>, global_config: &Config) -> Result<Vec<JwtAudience>> {
    let Some(value) = requested else {
        return Ok(global_config.jwt_audiences.clone());
    };

    if !global_config
        .allowed_resource_audiences
        .iter()
        .any(|allowed| allowed == value)
    {
        return Err(anyhow!(
            "invalid_target: '{value}' not in allowed audiences"
        ));
    }

    JwtAudience::from_str(value)
        .map(|aud| vec![aud])
        .map_err(|e| anyhow!("Invalid audience '{value}': {e}"))
}
