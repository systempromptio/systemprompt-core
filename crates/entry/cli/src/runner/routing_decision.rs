//! Where a routed command runs once the execution target is known.
//!
//! [`decide_routing`] turns the resolved target into one [`RoutingDecision`]:
//! a remote run, or a local continuation the profile and the command's
//! [`RoutingClass`] / [`DataImpact`] allow. A cloud profile that cannot route
//! never silently falls back for a destructive command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, bail};

use crate::descriptor::{DataImpact, RoutingClass};

/// Where a command runs once the routing target is known.
#[derive(Debug, PartialEq, Eq)]
pub enum RoutingDecision {
    ExecuteRemote {
        hostname: String,
        token: systemprompt_identifiers::SessionToken,
        context: systemprompt_identifiers::ContextId,
    },
    ContinueLocal,
}

pub fn decide_routing(
    target: Result<super::routing::ExecutionTarget>,
    profile: &systemprompt_models::Profile,
    class: RoutingClass,
    impact: DataImpact,
) -> Result<RoutingDecision> {
    use super::routing::ExecutionTarget;

    let is_cloud = profile.target.is_cloud();
    match target {
        Ok(ExecutionTarget::Remote {
            hostname,
            token,
            context,
        }) => Ok(RoutingDecision::ExecuteRemote {
            hostname,
            token,
            context,
        }),
        Ok(ExecutionTarget::Local) if is_cloud => {
            allow_local_execution(profile, class, "no tenant is configured")?;
            Ok(RoutingDecision::ContinueLocal)
        },
        Err(e) if is_cloud => {
            let reason = format!("routing failed: {}", e);
            if impact == DataImpact::Destructive {
                bail!(
                    "Cloud profile '{}' could not route this destructive command remotely ({}); \
                     it never falls back to direct database access.\n{}",
                    profile.name,
                    reason,
                    remediation_for(&reason)
                );
            }
            allow_local_execution(profile, class, &reason)?;
            Ok(RoutingDecision::ContinueLocal)
        },
        Ok(ExecutionTarget::Local) => Ok(RoutingDecision::ContinueLocal),
        Err(e) => {
            tracing::debug!(error = %e, "Routing failed on a local profile; continuing locally");
            Ok(RoutingDecision::ContinueLocal)
        },
    }
}

pub fn allow_local_execution(
    profile: &systemprompt_models::Profile,
    class: RoutingClass,
    reason: &str,
) -> Result<()> {
    if profile.database.external_db_access {
        tracing::debug!(
            profile_name = %profile.name,
            reason = reason,
            "Cloud profile allowing local execution via external_db_access"
        );
        return Ok(());
    }

    if class == RoutingClass::ReadOnly {
        tracing::warn!(
            profile_name = %profile.name,
            reason = reason,
            "Cloud profile could not route remotely; reading local data instead"
        );
        return Ok(());
    }

    bail!(
        "Cloud profile '{}' requires remote execution but {}.\n{}",
        profile.name,
        reason,
        remediation_for(reason)
    )
}

pub fn remediation_for(reason: &str) -> &'static str {
    if reason.contains("load tenants") || reason.contains("tenant") {
        "Run 'systemprompt cloud tenant list' to sync the tenant store, and check you are in the \
         project directory this profile belongs to."
    } else {
        "Run 'systemprompt admin session login' to authenticate."
    }
}
