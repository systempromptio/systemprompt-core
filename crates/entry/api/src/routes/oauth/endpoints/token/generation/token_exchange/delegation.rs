//! Resolves who an exchanged token is issued for: the resource it may target,
//! the delegate and permission ceiling, and the session it is bound to.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::str::FromStr;

use anyhow::{Result, anyhow};
use systemprompt_identifiers::{ClientId, SessionId, UserId};
use systemprompt_models::Config;
use systemprompt_models::auth::{Permission, parse_permissions};
use systemprompt_oauth::OAuthState;
use systemprompt_oauth::repository::OAuthRepository;
use systemprompt_oauth::services::validation::id_jag::resolve_bound_resource;
use systemprompt_oauth::services::{LinkedSubject, link_enterprise_principal};

use super::super::super::TokenError;
use super::super::RequestOrigin;
use super::claims::intersect_scopes;
use super::subject::SubjectIdentity;

#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "items are re-exported via `test_api` only when the feature is on"
    )
)]
pub fn validate_resource<'a>(
    resource: Option<&'a str>,
    global: &Config,
) -> Result<Option<&'a str>> {
    match resource {
        Some(value)
            if !global
                .allowed_resource_audiences
                .iter()
                .any(|allowed| allowed == value) =>
        {
            Err(anyhow!(TokenError::InvalidTarget {
                message: format!("'{value}' not in allowed_resource_audiences"),
            }))
        },
        other => Ok(other),
    }
}

// Why: Resolve the resource the issued token may target, honouring an ID-JAG's
// pin before the deployment's own allowlist.
pub(super) fn resolve_resource(
    subject: &SubjectIdentity,
    requested: Option<&str>,
    global: &Config,
) -> Result<Option<String>> {
    let effective =
        resolve_bound_resource(subject.bound_resource.as_deref(), requested).map_err(|e| {
            anyhow!(TokenError::InvalidTarget {
                message: e.to_string(),
            })
        })?;
    Ok(validate_resource(effective, global)?.map(ToOwned::to_owned))
}

// Why: Resolve who the token is issued for, and the permissions it may carry.
// An ID-JAG subject names an employee, so the ceiling is that employee's
// permissions; every other subject delegates the client owner's.
pub(super) async fn resolve_delegate(
    repo: &OAuthRepository,
    state: &OAuthState,
    client_id: &ClientId,
    subject: &SubjectIdentity,
    requested_scope: Option<&str>,
) -> Result<(LinkedSubject, Vec<Permission>)> {
    let grant = load_delegation_grant(repo, state, client_id).await?;
    let delegate = match subject.principal.as_ref() {
        Some(principal) => link_enterprise_principal(state, principal).await?,
        None => LinkedSubject {
            user_id: grant.owner_user_id,
            name: grant.owner_name,
            email: grant.owner_email,
            permissions: grant.owner_perms,
        },
    };

    let requested_perms = match requested_scope {
        Some(s) => parse_permissions(s)?,
        None => subject.scope.clone(),
    };
    let final_perms = intersect_scopes(
        &requested_perms,
        &subject.scope,
        &grant.client_perms,
        &delegate.permissions,
    )?;

    Ok((delegate, final_perms))
}

struct DelegationGrant {
    owner_user_id: UserId,
    owner_name: String,
    owner_email: String,
    owner_perms: Vec<Permission>,
    client_perms: Vec<Permission>,
}

async fn load_delegation_grant(
    repo: &OAuthRepository,
    state: &OAuthState,
    client_id: &ClientId,
) -> Result<DelegationGrant> {
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
    let owner_perms = owner
        .roles
        .iter()
        .filter_map(|r| Permission::from_str(r).ok())
        .collect();
    let client_perms = client
        .scopes
        .iter()
        .filter_map(|s| Permission::from_str(s).ok())
        .collect();

    Ok(DelegationGrant {
        owner_user_id: client.owner_user_id,
        owner_name: owner.name,
        owner_email: owner.email,
        owner_perms,
        client_perms,
    })
}

pub(super) async fn ensure_session(
    state: &OAuthState,
    origin: RequestOrigin<'_>,
    user_id: &UserId,
    global: &Config,
) -> Result<SessionId> {
    use systemprompt_identifiers::SessionSource;
    use systemprompt_traits::{CreateSessionInput, ExtractSignals};

    let session_id = SessionId::new(format!("sess_{}", uuid::Uuid::new_v4().simple()));
    let expires_at =
        chrono::Utc::now() + chrono::Duration::seconds(global.jwt_access_token_expiration);
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
            user_id: Some(user_id),
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
