//! Scope, audience, and `act`-chain resolution for token exchange.

use std::str::FromStr;

use anyhow::{Result, anyhow};
use systemprompt_identifiers::ClientId;
use systemprompt_models::Config;
use systemprompt_models::auth::{ActClaim, JwtAudience, Permission};

use super::super::super::TokenError;

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
            field: "scope".to_owned(),
            message: "no overlap between subject, client, and owner permissions".to_owned(),
        }));
    }
    Ok(out)
}

pub(super) fn resolve_audience(
    requested: Option<&str>,
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
    Ok(global.jwt_audiences.clone())
}

pub fn build_act_chain(client_id: &ClientId, issuer: &str, prior: Option<ActClaim>) -> ActClaim {
    ActClaim {
        iss: issuer.to_owned(),
        sub: client_id.to_string(),
        act: Box::new(prior),
    }
}
