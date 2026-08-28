//! Scope and audience authorization for the `client_credentials` grant.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::str::FromStr;
use systemprompt_models::Config;
use systemprompt_models::auth::{JwtAudience, Permission, permissions_to_string};

use super::ClientCredentialsError;

#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "items are re-exported via `test_api` only when the feature is on"
    )
)]
pub fn scope_permissions(scopes: &[String]) -> Vec<Permission> {
    scopes
        .iter()
        .filter_map(|s| Permission::from_str(s).ok())
        .collect()
}

// Why: service-tier scopes ([`Permission::is_service_scope`]) need only the
// client grant, but user-tier roles are delegated authority and require both
// the client *and* its owner to hold the permission — the RFC 6749 §4.4
// owner is audit attribution, never authorization by itself.
#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "items are re-exported via `test_api` only when the feature is on"
    )
)]
pub fn authorize_client_grant(
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

#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "items are re-exported via `test_api` only when the feature is on"
    )
)]
pub fn resolve_audience(
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
