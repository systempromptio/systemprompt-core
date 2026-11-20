//! Access- and refresh-token minting for a user-bound grant
//! (`authorization_code` and `refresh_token`).
//!
//! Resolves the granted permissions against the user's own, binds an OAuth
//! session, signs the access token and stores the rotated refresh token,
//! carrying the refresh-token family forward when one is given.
//!
//! A grant for a client the operator declared `person_ui` mints the person's
//! UI session instead (see [`systemprompt_models::auth::ui_session`]): exactly
//! the `ui` audience, the `ui_session` claim, a short access lifetime, the
//! sign-in time as `auth_time`, and a refresh token that never outlives the
//! absolute cap counted from that sign-in.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use systemprompt_identifiers::{
    AccessTokenId, ClientId, RefreshTokenId, SessionId, SessionSource, UserId,
};
use systemprompt_manifest::Config;
use systemprompt_models::auth::ui_session::{UI_SESSION_ABSOLUTE_SECS, UI_SESSION_ACCESS_TTL_SECS};
use systemprompt_models::auth::{
    AuthenticatedUser, JwtAudience, Permission, parse_permissions, permissions_to_string,
};
use systemprompt_models::errors::ParseEnumError;
use systemprompt_oauth::repository::{OAuthRepository, RefreshTokenLineage, RefreshTokenParams};
use systemprompt_oauth::services::{
    JwtConfig, JwtSigningParams, SessionCreationError, SessionCreationService, generate_jwt,
    generate_secure_token, load_interactive_user,
};
use systemprompt_oauth::{OAuthState, OauthError};
use systemprompt_traits::ExtractSignals;
use thiserror::Error;

use crate::{RequestOrigin, TokenResponse};

/// Failure minting a user-bound token pair.
#[derive(Debug, Error)]
pub enum UserTokenError {
    #[error("Scope is required for token generation")]
    MissingScope,
    #[error("Requested scope is not a list of known permissions")]
    UnparseableScope(#[from] ParseEnumError),
    #[error("No valid permissions available for user")]
    NoPermissions,
    #[error("Failed to create session")]
    Session(#[from] SessionCreationError),
    #[error("A UI session is bound to the `ui` audience and takes no resource")]
    UiSessionResource,
    #[error("The UI session reached its absolute lifetime; the person must sign in again")]
    UiSessionExpired,
    #[error(transparent)]
    Oauth(#[from] OauthError),
}

#[derive(Debug)]
pub struct UserTokenParams<'a> {
    pub client_id: &'a ClientId,
    pub user_id: &'a UserId,
    pub scope: Option<&'a str>,
    pub origin: RequestOrigin<'a>,
    pub resource: Option<&'a str>,
    pub refresh_token: &'a MintedRefreshToken,
    pub lineage: RefreshTokenLineage<'a>,
    pub authenticated_at: DateTime<Utc>,
    pub ui_session: bool,
    pub config: &'a Arc<Config>,
}

/// A refresh token minted before the grant is redeemed, so the redemption can
/// record it in the same statement that claims the grant.
#[derive(Debug)]
pub struct MintedRefreshToken {
    value: String,
    id: RefreshTokenId,
}

impl MintedRefreshToken {
    pub fn mint() -> Self {
        let value = generate_secure_token("rt");
        let id = RefreshTokenId::new(&value);
        Self { value, id }
    }

    pub const fn id(&self) -> &RefreshTokenId {
        &self.id
    }
}

#[derive(Debug)]
pub struct GeneratedTokens {
    pub response: TokenResponse,
    pub refresh_token_id: RefreshTokenId,
}

pub async fn generate_tokens_by_user_id(
    repo: &OAuthRepository,
    params: UserTokenParams<'_>,
    state: &OAuthState,
) -> Result<GeneratedTokens, UserTokenError> {
    let config = params.config;
    let shape = GrantShape::resolve(&params, Utc::now())?;

    let scope_str = params.scope.ok_or(UserTokenError::MissingScope)?;

    let user = load_interactive_user(state.user_provider().as_ref(), params.user_id).await?;

    let requested_permissions = parse_permissions(scope_str)?;
    let final_permissions = resolve_user_permissions(&requested_permissions, user.permissions())?;
    let session_service = SessionCreationService::new(
        Arc::clone(state.session_provider()),
        Arc::clone(state.user_provider()),
        Arc::clone(state.token_authority()),
        Arc::clone(config),
    );
    let analytics = state.analytics_provider().extract_analytics(
        params.origin.headers,
        ExtractSignals {
            caller_ip: params.origin.caller_ip,
            ..Default::default()
        },
    );
    let session_id = session_service
        .create_authenticated_session(params.user_id, &analytics, SessionSource::Oauth)
        .await?;

    let subject = TokenSubject {
        user: &user,
        permissions: final_permissions,
        session_id: &session_id,
    };
    let jwt_and_refresh = create_jwt_and_refresh_token(state, subject, &params, &shape).await?;

    if let Err(e) = repo.update_client_last_used(params.client_id).await {
        tracing::warn!(
            client_id = %params.client_id,
            error = %e,
            "Failed to update client last_used timestamp"
        );
    }

    Ok(GeneratedTokens {
        response: TokenResponse {
            access_token: jwt_and_refresh.access_token,
            token_type: "Bearer".to_owned(),
            expires_in: shape.access_ttl.num_seconds(),
            refresh_token: Some(jwt_and_refresh.refresh_token_value),
            scope: Some(jwt_and_refresh.scope_string),
            issued_token_type: None,
        },
        refresh_token_id: jwt_and_refresh.refresh_token_id,
    })
}

struct GrantShape {
    audience: Vec<JwtAudience>,
    access_ttl: Duration,
    refresh_expires_at: i64,
    auth_time: Option<i64>,
    browser_session: bool,
}

impl GrantShape {
    fn resolve(params: &UserTokenParams<'_>, now: DateTime<Utc>) -> Result<Self, UserTokenError> {
        let config = params.config.as_ref();
        let refresh_expires_at = now.timestamp() + config.jwt_refresh_token_expiration;
        if !params.ui_session {
            return Ok(Self {
                audience: config.jwt_audiences.clone(),
                access_ttl: Duration::seconds(config.jwt_access_token_expiration),
                refresh_expires_at,
                auth_time: None,
                browser_session: *params.client_id == ClientId::web(),
            });
        }
        if params.resource.is_some() {
            return Err(UserTokenError::UiSessionResource);
        }
        let session_ends = params.authenticated_at.timestamp() + UI_SESSION_ABSOLUTE_SECS;
        let remaining = session_ends - now.timestamp();
        if remaining <= 0 {
            return Err(UserTokenError::UiSessionExpired);
        }
        Ok(Self {
            audience: vec![JwtAudience::Ui],
            access_ttl: Duration::seconds(UI_SESSION_ACCESS_TTL_SECS.min(remaining)),
            refresh_expires_at: refresh_expires_at.min(session_ends),
            auth_time: Some(params.authenticated_at.timestamp()),
            browser_session: false,
        })
    }
}

struct TokenSubject<'a> {
    user: &'a AuthenticatedUser,
    permissions: Vec<Permission>,
    session_id: &'a SessionId,
}

struct JwtAndRefreshToken {
    access_token: String,
    refresh_token_value: String,
    scope_string: String,
    refresh_token_id: RefreshTokenId,
}

async fn create_jwt_and_refresh_token(
    state: &OAuthState,
    subject: TokenSubject<'_>,
    params: &UserTokenParams<'_>,
    shape: &GrantShape,
) -> Result<JwtAndRefreshToken, UserTokenError> {
    let repo = state.oauth_repository();
    let global_config = params.config.as_ref();
    let TokenSubject {
        user,
        permissions,
        session_id,
    } = subject;
    let scope_string = permissions_to_string(&permissions);
    let access_token_jti = AccessTokenId::generate();
    let config = JwtConfig {
        permissions,
        audience: shape.audience.clone(),
        resource: params.resource.map(String::from),
        expires_in: shape.access_ttl,
        plugin_id: None,
        client_id: Some(params.client_id.clone()),
        purpose: None,
        interaction: None,
        browser_session: shape.browser_session,
        ui_session: params.ui_session,
        auth_time: shape.auth_time,
    };
    let signing = JwtSigningParams {
        issuer: &global_config.jwt_issuer,
        keys: state.signing_keys(),
    };
    let access_token = generate_jwt(user, config, access_token_jti, session_id, &signing)?;

    repo.store_refresh_token(RefreshTokenParams {
        token_id: params.refresh_token.id(),
        client_id: params.client_id,
        user_id: params.user_id,
        scope: &scope_string,
        expires_at: shape.refresh_expires_at,
        resource: params.resource,
        authenticated_at: params.authenticated_at,
        lineage: params.lineage,
    })
    .await?;

    Ok(JwtAndRefreshToken {
        access_token,
        refresh_token_value: params.refresh_token.value.clone(),
        scope_string,
        refresh_token_id: params.refresh_token.id.clone(),
    })
}

pub fn resolve_user_permissions(
    requested_permissions: &[Permission],
    user_permissions: &[Permission],
) -> Result<Vec<Permission>, UserTokenError> {
    let mut final_permissions = Vec::new();

    for requested in requested_permissions {
        if *requested == Permission::User {
            final_permissions.extend(
                user_permissions
                    .iter()
                    .filter(|p| p.is_user_role())
                    .copied(),
            );
        } else if user_permissions.contains(requested) {
            final_permissions.push(*requested);
        }
    }

    final_permissions.sort_by_key(|p| std::cmp::Reverse(p.hierarchy_level()));
    final_permissions.dedup();

    if final_permissions.is_empty() {
        return Err(UserTokenError::NoPermissions);
    }

    Ok(final_permissions)
}
