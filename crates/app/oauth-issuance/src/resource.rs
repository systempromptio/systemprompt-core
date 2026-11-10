//! RFC 8707 resource resolution shared by every token grant.
//!
//! A grant either carries a resource the user already consented to (an
//! authorization code, or the refresh token it produced) or carries none
//! (client credentials, token exchange, jwt-bearer). A bound grant may be
//! redeemed for exactly its resource or for none; an unbound grant may name
//! only a resource in `allowed_resource_audiences`. A bound grant with no
//! resource is issued the default audiences and can never gain one.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_manifest::Config;

use crate::{IssuanceError, IssuanceResult};

/// The resource a grant was issued for, as recorded at issuance.
#[derive(Debug, Clone, Copy)]
pub enum ResourceBinding<'a> {
    Unbound,
    Bound(Option<&'a str>),
}

pub fn resolve_grant_resource<'a>(
    requested: Option<&'a str>,
    binding: ResourceBinding<'a>,
    global: &Config,
) -> IssuanceResult<Option<&'a str>> {
    match binding {
        ResourceBinding::Unbound => validate_resource(requested, global),
        ResourceBinding::Bound(bound) => match (requested, bound) {
            (None, bound) => Ok(bound),
            (Some(value), Some(bound)) if value == bound => Ok(Some(bound)),
            (Some(value), Some(bound)) => Err(IssuanceError::InvalidTarget {
                message: format!(
                    "'{value}' is not the resource '{bound}' this grant was issued for"
                ),
            }),
            (Some(value), None) => Err(IssuanceError::InvalidTarget {
                message: format!("'{value}' was not granted; this grant carries no resource"),
            }),
        },
    }
}

pub fn validate_resource<'a>(
    resource: Option<&'a str>,
    global: &Config,
) -> IssuanceResult<Option<&'a str>> {
    match resource {
        Some(value) if !is_allowed_resource(value, global) => Err(IssuanceError::InvalidTarget {
            message: format!("'{value}' not in allowed_resource_audiences"),
        }),
        other => Ok(other),
    }
}

pub fn is_allowed_resource(value: &str, global: &Config) -> bool {
    global
        .allowed_resource_audiences
        .iter()
        .any(|allowed| allowed == value)
}
