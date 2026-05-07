use anyhow::Result;
use axum::http::HeaderMap;
use sha2::{Digest, Sha256};
use std::str::FromStr;
use systemprompt_identifiers::{ClientId, SessionId, SessionSource, UserId};
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

    let scope_str = options
        .scope
        .ok_or_else(|| anyhow::anyhow!("Scope is required for client credentials grant"))?;

    let requested_permissions = parse_permissions(scope_str)?;

    let client = repo
        .find_client_by_id(client_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Client not found"))?;

    let permissions = resolve_client_permissions(requested_permissions, &client.scopes)?;
    let (user_id, client_user) = build_client_user(client_id, &permissions);

    let jwt_secret = systemprompt_config::SecretsBootstrap::jwt_secret()?;
    let global_config = Config::get()?;

    // Why: Resource audiences are intentionally open here. Capability is granted by
    // `scope`, not audience; the audience claim is informational and does not
    // authorize anything.
    let audience_override = options
        .audience
        .map(|a| {
            JwtAudience::from_str(a).map_err(|e| anyhow::anyhow!("Invalid audience '{a}': {e}"))
        })
        .transpose()?;
    let audience =
        audience_override.map_or_else(|| global_config.jwt_audiences.clone(), |aud| vec![aud]);

    if permissions.iter().any(Permission::is_hook_scope)
        && !audience.iter().any(|a| matches!(a, JwtAudience::Hook))
    {
        return Err(anyhow::anyhow!(
            "Hook scopes require audience=hook on the token request"
        ));
    }

    let config = JwtConfig {
        permissions: permissions.clone(),
        audience,
        expires_in_hours: Some(global_config.jwt_access_token_expiration / 3600),
        plugin_id: options.plugin_id.map(str::to_string),
        ..Default::default()
    };
    let session_id = SessionId::new(format!("sess_{}", uuid::Uuid::new_v4().simple()));
    let expires_at = chrono::Utc::now() + chrono::Duration::seconds(expires_in);
    let analytics = state.analytics_provider().extract_analytics(headers, None);

    state
        .analytics_provider()
        .create_session(CreateSessionInput {
            session_id: &session_id,
            user_id: Some(&user_id),
            analytics: &analytics,
            session_source: SessionSource::Oauth,
            is_bot: false,
            is_ai_crawler: false,
            expires_at,
        })
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create session: {e}"))?;

    let signing = JwtSigningParams {
        secret: jwt_secret,
        issuer: &global_config.jwt_issuer,
    };
    let jwt_token = generate_jwt(
        &client_user,
        config,
        uuid::Uuid::new_v4().to_string(),
        &session_id,
        &signing,
    )?;

    Ok(TokenResponse {
        access_token: jwt_token,
        token_type: "Bearer".to_string(),
        expires_in,
        refresh_token: None,
        scope: Some(
            permissions
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(" "),
        ),
    })
}

fn resolve_client_permissions(
    requested_permissions: Vec<Permission>,
    client_scopes: &[String],
) -> Result<Vec<Permission>> {
    let client_allowed: Vec<Permission> = client_scopes
        .iter()
        .filter_map(|s| {
            Permission::from_str(s)
                .map_err(|e| {
                    tracing::warn!(scope = %s, error = %e, "Invalid scope in client configuration");
                    e
                })
                .ok()
        })
        .collect();

    let permissions: Vec<Permission> = requested_permissions
        .into_iter()
        .filter(|p| client_allowed.contains(p))
        .collect();

    if permissions.is_empty() {
        return Err(anyhow::anyhow!(
            "No valid permissions: requested scopes not allowed for this client"
        ));
    }

    Ok(permissions)
}

fn build_client_user(
    client_id: &ClientId,
    permissions: &[Permission],
) -> (UserId, AuthenticatedUser) {
    let client_id_str = client_id.as_str();
    let mut hasher = Sha256::new();
    hasher.update(format!("client.{client_id_str}").as_bytes());
    let hash = hasher.finalize();

    let mut uuid_bytes = [0u8; 16];
    uuid_bytes.copy_from_slice(&hash[..16]);
    let client_uuid = uuid::Uuid::from_bytes(uuid_bytes);
    let user_id = UserId::new(client_uuid.to_string());

    let role_strings: Vec<String> = permissions.iter().map(ToString::to_string).collect();
    let client_user = AuthenticatedUser::new_with_roles(
        client_uuid,
        format!("client:{client_id_str}"),
        format!("{client_id_str}@client.local"),
        permissions.to_vec(),
        role_strings,
    );

    (user_id, client_user)
}
