//! Decides whether a command runs locally or is forwarded to a remote tenant.
//!
//! [`determine_execution_target`] resolves the active profile and the cloud
//! member's tenant store into an [`ExecutionTarget`]; a profile that names a
//! tenant in a build without the cloud member is refused
//! ([`CloudAccessError::Absent`](systemprompt_extension::cli::CloudAccessError)); [`execute_remote`] adapts the terminal to the
//! SSE transport in `systemprompt_client::RemoteCliExecutor`.
//!
//! A managed tenant routes with the stored `admin session login` session. A
//! self-managed (kubernetes/gke) tenant resolves to
//! [`ExecutionTarget::SelfManaged`]; `authorize_target` turns it into a
//! remote target by exchanging the operator's OIDC sign-in for a tenant
//! session at the tenant's token endpoint, so no database URL or signing key
//! is needed on the operator's machine.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io::{self, Write};

use anyhow::{Context, Result};
use systemprompt_client::{OutputSink, RemoteCliExecutor, RemoteCliRequest};
use systemprompt_extension::cli::{DynCloudAccess, TenantRoute};
use systemprompt_identifiers::{ContextId, SessionToken};
use systemprompt_loader::workspace::{SessionKey, SessionStore};
use systemprompt_logging::CliService;
use systemprompt_manifest::Profile;

use systemprompt_loader::workspace::paths::ResolvedPaths;

#[doc(hidden)]
#[derive(Debug)]
pub enum ExecutionTarget {
    Local,
    Remote {
        hostname: String,
        token: SessionToken,
        context: Option<ContextId>,
    },
    SelfManaged {
        hostname: String,
    },
}

pub fn determine_execution_target(
    profile: &Profile,
    cloud: Option<&DynCloudAccess>,
) -> Result<ExecutionTarget> {
    if profile.target.is_local() {
        tracing::debug!(
            profile_name = %profile.name,
            "Profile target is local, routing to local execution"
        );
        return Ok(ExecutionTarget::Local);
    }

    let Some(tenant_id) = profile.cloud.as_ref().and_then(|c| c.tenant_id.as_ref()) else {
        tracing::debug!(
            profile_name = %profile.name,
            "Profile has no tenant_id, routing to local execution"
        );
        return Ok(ExecutionTarget::Local);
    };

    tracing::debug!(
        profile_name = %profile.name,
        tenant_id = %tenant_id,
        "Profile has tenant_id, resolving remote execution target"
    );

    let tenant = resolve_tenant(profile, tenant_id, cloud)?;
    let hostname = tenant
        .hostname
        .as_ref()
        .context("Tenant has no hostname configured")?
        .clone();

    if tenant.self_managed {
        tracing::debug!(
            hostname = %hostname,
            tenant_id = %tenant_id,
            provider = %tenant.provider,
            "Self-managed tenant, routing through the operator sign-in"
        );
        return Ok(ExecutionTarget::SelfManaged { hostname });
    }

    let session_key = SessionKey::Tenant(tenant_id.clone());
    let session = load_session_for_key(profile, &session_key, &profile.security.issuer)?;

    tracing::info!(
        hostname = %hostname,
        tenant_id = %tenant_id,
        "Routing to remote execution"
    );

    Ok(ExecutionTarget::Remote {
        hostname,
        token: session.session_token,
        context: session.context_id,
    })
}

pub(crate) async fn authorize_target(
    target: ExecutionTarget,
    profile: &Profile,
    cloud: Option<&DynCloudAccess>,
) -> Result<ExecutionTarget> {
    let ExecutionTarget::SelfManaged { hostname } = target else {
        return Ok(target);
    };
    let token = crate::cloud_access::require_cloud(
        cloud,
        Some(profile),
        "signing in to a self-managed tenant",
    )?
    .operator_tenant_token(&hostname)
    .await?;
    Ok(ExecutionTarget::Remote {
        hostname,
        token,
        context: Some(ContextId::generate()),
    })
}

pub fn resolve_tenant(
    profile: &Profile,
    tenant: &systemprompt_identifiers::TenantId,
    cloud: Option<&DynCloudAccess>,
) -> Result<TenantRoute> {
    let cloud = crate::cloud_access::require_cloud(
        cloud,
        Some(profile),
        "routing a command to a cloud tenant",
    )?;
    let tenants_path = ResolvedPaths::from_profile(profile).tenants_path();
    Ok(cloud.tenant_route(&tenants_path, tenant)?)
}

pub fn load_session_for_key(
    profile: &Profile,
    session_key: &SessionKey,
    issuer: &str,
) -> Result<systemprompt_loader::workspace::CliSession> {
    let sessions_dir = ResolvedPaths::from_profile(profile).sessions_dir();

    let store = SessionStore::load_or_create(&sessions_dir)?;

    store
        .get_valid_session(session_key, issuer)
        .cloned()
        .context("No active session. Run 'systemprompt admin session login'.")
}

struct StdioSink {
    stdout: io::Stdout,
    stderr: io::Stderr,
}

impl OutputSink for StdioSink {
    fn stdout_chunk(&mut self, data: &str) -> io::Result<()> {
        write!(self.stdout, "{}", data)?;
        self.stdout.flush()
    }

    fn stderr_chunk(&mut self, data: &str) -> io::Result<()> {
        write!(self.stderr, "{}", data)?;
        self.stderr.flush()
    }

    fn error_message(&mut self, message: &str) {
        CliService::error(message);
    }
}

pub async fn execute_remote(
    base_url: &str,
    token: &SessionToken,
    context: Option<&ContextId>,
    args: &[String],
    timeout_secs: u64,
) -> Result<i32> {
    let executor =
        RemoteCliExecutor::new(base_url, timeout_secs).context("Failed to create HTTP client")?;
    let mut sink = StdioSink {
        stdout: io::stdout(),
        stderr: io::stderr(),
    };
    let request = RemoteCliRequest {
        token,
        context,
        args,
    };
    Ok(executor.execute(request, &mut sink).await?)
}
