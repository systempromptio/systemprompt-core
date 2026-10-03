//! Resolves who an exchanged token is issued for: the resource it may target,
//! the delegate and permission ceiling, and the session it is bound to.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::str::FromStr;

use systemprompt_identifiers::{ClientId, SessionId, UserId};
use systemprompt_manifest::Config;
use systemprompt_models::auth::{Permission, parse_permissions};
use systemprompt_oauth::OAuthState;
use systemprompt_oauth::repository::OAuthRepository;
use systemprompt_oauth::services::validation::id_jag::resolve_bound_resource;
use systemprompt_oauth::services::{LinkedSubject, link_enterprise_principal};

use super::claims::intersect_scopes;
use super::subject::SubjectIdentity;
use crate::session::create_oauth_session;
use crate::{IssuanceError, IssuanceResult, RequestOrigin};

pub fn validate_resource<'a>(
    resource: Option<&'a str>,
    global: &Config,
) -> IssuanceResult<Option<&'a str>> {
    match resource {
        Some(value)
            if !global
                .allowed_resource_audiences
                .iter()
                .any(|allowed| allowed == value) =>
        {
            Err(IssuanceError::InvalidTarget {
                message: format!("'{value}' not in allowed_resource_audiences"),
            })
        },
        other => Ok(other),
    }
}

pub(super) fn resolve_resource(
    subject: &SubjectIdentity,
    requested: Option<&str>,
    global: &Config,
) -> IssuanceResult<Option<String>> {
    let effective = resolve_bound_resource(subject.bound_resource.as_deref(), requested)
        .map_err(IssuanceError::BoundResource)?;
    Ok(validate_resource(effective, global)?.map(ToOwned::to_owned))
}

pub(super) async fn resolve_delegate(
    repo: &OAuthRepository,
    state: &OAuthState,
    client_id: &ClientId,
    subject: &SubjectIdentity,
    requested_scope: Option<&str>,
) -> IssuanceResult<(LinkedSubject, Vec<Permission>)> {
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
        Some(s) => parse_permissions(s).map_err(|_unknown| IssuanceError::InvalidScope {
            message: "scope contains an unknown permission".to_owned(),
        })?,
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
) -> IssuanceResult<DelegationGrant> {
    let client = repo
        .find_client_by_id(client_id)
        .await?
        .ok_or(IssuanceError::InvalidClient)?;
    let owner = state
        .user_provider()
        .find_by_id(&client.owner_user_id)
        .await
        .map_err(|e| IssuanceError::server("Failed to load client owner", e))?
        .ok_or(IssuanceError::InvalidClient)?;
    if !owner.is_active {
        return Err(IssuanceError::InvalidClient);
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
) -> IssuanceResult<SessionId> {
    create_oauth_session(state, origin, user_id, global.jwt_access_token_expiration)
        .await
        .map_err(|e| IssuanceError::server("Failed to create session", e))
}
