//! Decides whether a command runs locally or is forwarded to a remote tenant.
//!
//! [`determine_execution_target`] resolves the active profile and tenant store
//! into an [`ExecutionTarget`]; [`execute_remote`] adapts the terminal to the
//! SSE transport in `systemprompt_client::RemoteCliExecutor`.

use std::io::{self, Write};

use anyhow::{Context, Result};
use systemprompt_client::{OutputSink, RemoteCliExecutor, RemoteCliRequest};
use systemprompt_cloud::{SessionKey, SessionStore, StoredTenant, TenantStore};
use systemprompt_config::ProfileBootstrap;
use systemprompt_identifiers::{ContextId, SessionToken};
use systemprompt_logging::CliService;

use crate::paths::ResolvedPaths;

pub(super) enum ExecutionTarget {
    Local,
    Remote {
        hostname: String,
        token: SessionToken,
        context: ContextId,
    },
}

pub(super) fn determine_execution_target() -> Result<ExecutionTarget> {
    let Ok(profile) = ProfileBootstrap::get() else {
        tracing::debug!("No profile loaded, routing to local execution");
        return Ok(ExecutionTarget::Local);
    };

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

    let tenant = resolve_tenant(tenant_id)?;
    let hostname = tenant
        .hostname
        .as_ref()
        .context("Tenant has no hostname configured")?
        .clone();

    let session_key = SessionKey::Tenant(tenant_id.clone());
    let session = load_session_for_key(&session_key)?;

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

fn resolve_tenant(tenant: &systemprompt_identifiers::TenantId) -> Result<StoredTenant> {
    let tenants_path = ResolvedPaths::discover().tenants_path();

    let store = TenantStore::load_from_path(&tenants_path)
        .context("Failed to load tenants. Run 'systemprompt cloud tenant list' to sync.")?;

    store
        .find_tenant(tenant)
        .cloned()
        .with_context(|| format!("Tenant '{}' not found in local tenant store", tenant))
}

fn load_session_for_key(session_key: &SessionKey) -> Result<systemprompt_cloud::CliSession> {
    let sessions_dir = ResolvedPaths::discover().sessions_dir();

    let store = SessionStore::load_or_create(&sessions_dir)?;

    store
        .get_valid_session(session_key)
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

pub(in crate::runner) async fn execute_remote(
    hostname: &str,
    token: &str,
    context: &str,
    args: &[String],
    timeout_secs: u64,
) -> Result<i32> {
    let executor = RemoteCliExecutor::new(&format!("https://{hostname}"), timeout_secs)
        .context("Failed to create HTTP client")?;
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
