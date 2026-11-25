//! Where a gateway route def may land.
//!
//! A gateway route def is confined to the gateway path its set claims: its
//! gateway-relative path lies within the claim, and no built-in gateway
//! prefix covers it ([`refused_gateway_def`]).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::base_ref::GatewayPath;
use super::overlap::path_covers;
use super::shape::RouteDef;
use crate::reserved::BUILTIN_GATEWAY_PREFIXES;
use systemprompt_models::modules::ApiPaths;

/// Why a gateway route def is refused at the gateway path its set claims.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GatewayDefRefusal {
    UnderBuiltin { path: String, builtin: &'static str },
    OutsideClaim { path: String, claimed: &'static str },
}

#[must_use]
pub fn refused_gateway_def(claimed: GatewayPath, def: &RouteDef) -> Option<GatewayDefRefusal> {
    let path = def.path.as_str();
    if let Some(builtin) = BUILTIN_GATEWAY_PREFIXES
        .iter()
        .copied()
        .find(|builtin| path_covers(builtin, path))
    {
        return Some(GatewayDefRefusal::UnderBuiltin {
            path: path.to_owned(),
            builtin,
        });
    }
    let claim = ApiPaths::gateway_relative(claimed.path());
    (!path_covers(claim, path)).then(|| GatewayDefRefusal::OutsideClaim {
        path: path.to_owned(),
        claimed: claim,
    })
}
