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
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use systemprompt_identifiers::{ClientId, SessionId, UserId};
use systemprompt_models::Config;
use systemprompt_models::auth::{
    ActClaim, AuthenticatedUser, JwtAudience, JwtClaims, Permission, parse_permissions,
};
use systemprompt_models::profile::TrustedIssuer;
use systemprompt_oauth::OAuthState;
use systemprompt_oauth::repository::OAuthRepository;
use systemprompt_oauth::services::{JwtConfig, JwtSigningParams, generate_jwt_with_act};
use systemprompt_security::keys::JwksClient;

use super::super::{TokenError, TokenResponse};

#[derive(serde::Deserialize)]
struct IssOnly {
    iss: String,
}

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

/// Outcome of validating a subject token: original scope and any
/// pre-existing `act` chain to extend.
struct SubjectIdentity {
    scope: Vec<Permission>,
    prior_act: Option<ActClaim>,
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
    let jwt_secret = systemprompt_config::SecretsBootstrap::jwt_secret()?;
    let signing = JwtSigningParams {
        secret: jwt_secret,
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

async fn validate_subject_token(
    token: &str,
    token_type: &str,
    global: &Config,
) -> Result<SubjectIdentity> {
    if !matches!(
        token_type,
        ACCESS_TOKEN_TYPE | ID_TOKEN_TYPE | JWT_TOKEN_TYPE
    ) {
        return Err(anyhow!(TokenError::InvalidRequest {
            field: "subject_token_type".to_string(),
            message: format!("unsupported subject_token_type '{token_type}'"),
        }));
    }

    let header = decode_header(token).map_err(|e| {
        anyhow!(TokenError::InvalidRequest {
            field: "subject_token".to_string(),
            message: format!("malformed JWT header: {e}"),
        })
    })?;

    let declared_iss = peek_issuer(token)?;

    if declared_iss == global.jwt_issuer {
        return validate_self_issued(token, global);
    }

    let trusted = global
        .trusted_issuers
        .iter()
        .find(|t| t.issuer == declared_iss)
        .ok_or_else(|| {
            anyhow!(TokenError::InvalidRequest {
                field: "subject_token".to_string(),
                message: format!("issuer '{declared_iss}' is not trusted"),
            })
        })?;

    let kid = header.kid.ok_or_else(|| {
        anyhow!(TokenError::InvalidRequest {
            field: "subject_token".to_string(),
            message: "trusted-issuer token must carry a kid header".to_string(),
        })
    })?;

    let allowed_hosts = jwks_host_allowlist(&global.trusted_issuers);
    let client = JwksClient::new(allowed_hosts);
    let jwk = client
        .fetch_at(&trusted.issuer, &trusted.jwks_uri, &kid)
        .await
        .map_err(|e| {
            anyhow!(TokenError::InvalidRequest {
                field: "subject_token".to_string(),
                message: format!("JWKS resolution failed: {e}"),
            })
        })?;

    let decoding_key = DecodingKey::from_rsa_components(&jwk.n, &jwk.e).map_err(|e| {
        anyhow!(TokenError::InvalidRequest {
            field: "subject_token".to_string(),
            message: format!("invalid RSA components in JWK: {e}"),
        })
    })?;

    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&[&trusted.issuer]);
    validation.set_audience(&[&trusted.audience]);

    let data = decode::<JwtClaims>(token, &decoding_key, &validation).map_err(|e| {
        anyhow!(TokenError::InvalidRequest {
            field: "subject_token".to_string(),
            message: format!("subject token signature/claims rejected: {e}"),
        })
    })?;

    Ok(SubjectIdentity {
        scope: data.claims.scope,
        prior_act: data.claims.act,
    })
}

/// Decode the `iss` claim without validating the signature.
///
/// The result is only used to route the token to the correct
/// signature-verification path; the actual `iss` and signature are
/// re-validated downstream.
pub fn peek_issuer(token: &str) -> Result<String> {
    use base64::Engine;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;

    let mut parts = token.split('.');
    let _header = parts.next();
    let payload = parts.next().ok_or_else(|| {
        anyhow!(TokenError::InvalidRequest {
            field: "subject_token".to_string(),
            message: "subject_token is not a JWT".to_string(),
        })
    })?;
    let bytes = URL_SAFE_NO_PAD.decode(payload).map_err(|e| {
        anyhow!(TokenError::InvalidRequest {
            field: "subject_token".to_string(),
            message: format!("subject_token payload is not base64url: {e}"),
        })
    })?;
    let parsed: IssOnly = serde_json::from_slice(&bytes).map_err(|e| {
        anyhow!(TokenError::InvalidRequest {
            field: "subject_token".to_string(),
            message: format!("subject_token payload missing iss: {e}"),
        })
    })?;
    Ok(parsed.iss)
}

fn validate_self_issued(token: &str, global: &Config) -> Result<SubjectIdentity> {
    let secret = systemprompt_config::SecretsBootstrap::jwt_secret()?;
    let mut validation = Validation::new(Algorithm::HS256);
    validation.set_issuer(&[&global.jwt_issuer]);
    let aud_strs: Vec<&str> = global
        .jwt_audiences
        .iter()
        .map(JwtAudience::as_str)
        .collect();
    validation.set_audience(&aud_strs);
    let data = decode::<JwtClaims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map_err(|e| {
        anyhow!(TokenError::InvalidGrant {
            reason: format!("subject_token rejected: {e}"),
        })
    })?;
    Ok(SubjectIdentity {
        scope: data.claims.scope,
        prior_act: data.claims.act,
    })
}

fn jwks_host_allowlist(trusted: &[TrustedIssuer]) -> Vec<String> {
    trusted
        .iter()
        .filter_map(|t| url::Url::parse(&t.jwks_uri).ok())
        .filter_map(|u| u.host_str().map(str::to_string))
        .collect()
}

/// Compute the effective permission set for a delegated token.
///
/// Requested permissions are filtered to those that exist in the subject
/// token, the authenticated client's scope grant, and the client owner's
/// role set. An empty client scope is treated as "no restriction beyond
/// owner". Returned in descending hierarchy order. Errors when the
/// intersection is empty so the caller can reject with `invalid_scope`.
pub fn intersect_scopes(
    requested: &[Permission],
    subject_scope: &[Permission],
    client_scope: &[Permission],
    owner_scope: &[Permission],
) -> Result<Vec<Permission>> {
    let mut out: Vec<Permission> = requested
        .iter()
        .filter(|p| subject_scope.contains(p))
        .filter(|p| client_scope.is_empty() || client_scope.contains(p))
        .filter(|p| owner_scope.contains(p))
        .copied()
        .collect();
    out.sort_by_key(|p| std::cmp::Reverse(p.hierarchy_level()));
    out.dedup();
    if out.is_empty() {
        return Err(anyhow!(TokenError::InvalidRequest {
            field: "scope".to_string(),
            message: "no overlap between subject, client, and owner permissions".to_string(),
        }));
    }
    Ok(out)
}

fn resolve_audience(
    requested: Option<&str>,
    resource: Option<&str>,
    global: &Config,
) -> Result<Vec<JwtAudience>> {
    if let Some(value) = requested {
        if !global
            .allowed_resource_audiences
            .iter()
            .any(|allowed| allowed == value)
        {
            return Err(anyhow!(TokenError::InvalidTarget {
                message: format!("audience '{value}' not in allowed_resource_audiences"),
            }));
        }
        let aud =
            JwtAudience::from_str(value).map_err(|e| anyhow!("Invalid audience '{value}': {e}"))?;
        return Ok(vec![aud]);
    }
    let _ = resource;
    Ok(global.jwt_audiences.clone())
}

/// Append `client_id` as the outermost actor on the RFC 8693 `act` chain,
/// chaining `prior` (the subject token's existing `act`, if any) underneath
/// it. The result is always non-empty.
pub fn build_act_chain(client_id: &ClientId, issuer: &str, prior: Option<ActClaim>) -> ActClaim {
    ActClaim {
        iss: issuer.to_string(),
        sub: client_id.to_string(),
        act: Box::new(prior),
    }
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
