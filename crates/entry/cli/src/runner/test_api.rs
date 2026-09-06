//! Delegating wrappers that expose the runner's routing internals to the
//! separate test workspace.
//!
//! The runner keeps its surface to [`super::run`], so the routing decisions —
//! which command may run locally, which profile must reach a tenant, what a
//! failed lookup advises — are otherwise unreachable from outside the crate.
//! Every function here forwards to the private one and adds nothing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use systemprompt_cloud::{CliSession, SessionKey, StoredTenant};
use systemprompt_identifiers::TenantId;
use systemprompt_models::Profile;

use super::{args, profile_routing, routing};
use crate::cli_settings::CliConfig;
use crate::descriptor::RoutingClass;

pub use super::routing::ExecutionTarget;

pub fn determine_execution_target() -> Result<ExecutionTarget> {
    routing::determine_execution_target()
}

pub fn resolve_tenant(profile: &Profile, tenant: &TenantId) -> Result<StoredTenant> {
    routing::resolve_tenant(profile, tenant)
}

pub fn load_session_for_key(
    profile: &Profile,
    session_key: &SessionKey,
    issuer: &str,
) -> Result<CliSession> {
    routing::load_session_for_key(profile, session_key, issuer)
}

pub async fn execute_remote(
    hostname: &str,
    token: &str,
    context: &str,
    args: &[String],
    timeout_secs: u64,
) -> Result<i32> {
    routing::execute_remote(hostname, token, context, args, timeout_secs).await
}

pub const fn is_cloud_bypass_command(command: Option<&args::Commands>) -> bool {
    profile_routing::is_cloud_bypass_command(command)
}

pub fn confirm_remote_job_run(
    cli: &args::Cli,
    cli_config: &CliConfig,
    profile_name: &str,
    hostname: &str,
) -> Result<()> {
    profile_routing::confirm_remote_job_run(cli, cli_config, profile_name, hostname)
}

pub fn allow_local_execution(profile: &Profile, class: RoutingClass, reason: &str) -> Result<()> {
    profile_routing::allow_local_execution(profile, class, reason)
}

pub fn remediation_for(reason: &str) -> &'static str {
    profile_routing::remediation_for(reason)
}
