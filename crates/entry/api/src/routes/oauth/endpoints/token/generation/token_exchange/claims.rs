//! Scope, audience, and `act`-chain resolution for token exchange.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::str::FromStr;

use systemprompt_identifiers::ClientId;
use systemprompt_manifest::Config;
use systemprompt_models::auth::{ActClaim, JwtAudience, Permission};

use super::super::super::{TokenError, TokenResult};

pub fn intersect_scopes(
    requested: &[Permission],
    subject_scope: &[Permission],
    client_scope: &[Permission],
    owner_scope: &[Permission],
) -> TokenResult<Vec<Permission>> {
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
        return Err(TokenError::InvalidRequest {
            field: "scope".to_owned(),
            message: "no overlap between subject, client, and owner permissions".to_owned(),
        });
    }
    Ok(out)
}

pub fn resolve_audience(requested: Option<&str>, global: &Config) -> TokenResult<Vec<JwtAudience>> {
    if let Some(value) = requested {
        if !global
            .allowed_resource_audiences
            .iter()
            .any(|allowed| allowed == value)
        {
            return Err(TokenError::InvalidTarget {
                message: format!("audience '{value}' not in allowed_resource_audiences"),
            });
        }
        let aud = JwtAudience::from_str(value).map_err(|_unknown| TokenError::InvalidTarget {
            message: format!("audience '{value}' is not a known audience"),
        })?;
        return Ok(vec![aud]);
    }
    Ok(global.jwt_audiences.clone())
}

pub fn build_act_chain(client_id: &ClientId, issuer: &str, prior: Option<ActClaim>) -> ActClaim {
    ActClaim {
        iss: issuer.to_owned(),
        sub: client_id.to_string(),
        act: Box::new(prior),
    }
}
