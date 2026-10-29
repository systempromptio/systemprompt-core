//! Decides whether a command runs locally or is forwarded to a remote tenant.
//!
//! [`determine_execution_target`] resolves the active profile and tenant store
//! into an [`ExecutionTarget`]; [`execute_remote`] adapts the terminal to the
//! SSE transport in `systemprompt_client::RemoteCliExecutor`.
//!
//! A managed tenant routes with the stored `admin session login` session. A
//! self-managed (kubernetes/gke) tenant resolves to
//! [`ExecutionTarget::SelfManaged`]; [`authorize_target`] turns it into a
//! remote target by exchanging the operator's OIDC sign-in for a tenant
//! session at the tenant's token endpoint, so no database URL or signing key
//! is needed on the operator's machine.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io::{self, Write};

use anyhow::{Context, Result};
use systemprompt_client::{OutputSink, RemoteCliExecutor, RemoteCliRequest};
use systemprompt_cloud::deploy::provider::tenant_base_url;
use systemprompt_cloud::operator::{OperatorCredentials, exchange_for_tenant};
use systemprompt_cloud::{
    CloudPath, SessionKey, SessionStore, StoredTenant, TenantStore, get_cloud_paths,
};
use systemprompt_identifiers::{ContextId, SessionToken};
use systemprompt_logging::CliService;
use systemprompt_manifest::Profile;

use crate::paths::ResolvedPaths;

#[doc(hidden)]
#[derive(Debug)]
pub enum ExecutionTarget {
    Local,
    Remote {
        hostname: String,
        token: SessionToken,
        context: ContextId,
    },
    SelfManaged {
        hostname: String,
    },
}

pub fn determine_execution_target(profile: &Profile) -> Result<ExecutionTarget> {
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

    let tenant = resolve_tenant(profile, tenant_id)?;
    let hostname = tenant
        .hostname
        .as_ref()
        .context("Tenant has no hostname configured")?
        .clone();

    if tenant.provider.is_self_managed() {
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

pub async fn authorize_target(target: ExecutionTarget) -> Result<ExecutionTarget> {
    let ExecutionTarget::SelfManaged { hostname } = target else {
        return Ok(target);
    };
    let token = operator_tenant_token(&hostname).await?;
    Ok(ExecutionTarget::Remote {
        hostname,
        token,
        context: ContextId::generate(),
    })
}

pub async fn operator_tenant_token(hostname: &str) -> Result<SessionToken> {
    let path = get_cloud_paths().resolve(CloudPath::OperatorCredentials);
    let operator = OperatorCredentials::load_valid(&path)?;
    let session = exchange_for_tenant(&tenant_base_url(hostname), &operator)
        .await
        .with_context(|| format!("Operator token exchange with {hostname} failed"))?;
    Ok(SessionToken::new(session.access_token))
}

pub fn resolve_tenant(
    profile: &Profile,
    tenant: &systemprompt_identifiers::TenantId,
) -> Result<StoredTenant> {
    let tenants_path = ResolvedPaths::from_profile(profile).tenants_path();

    let store = TenantStore::load_from_path(&tenants_path).with_context(|| {
        format!(
            "Failed to load tenants from {}. Run 'systemprompt cloud tenant list' to sync.",
            tenants_path.display()
        )
    })?;

    store.find_tenant(tenant).cloned().with_context(|| {
        format!(
            "Tenant '{tenant}' not found in local tenant store. Run 'systemprompt cloud tenant \
                 list' to sync."
        )
    })
}

pub fn load_session_for_key(
    profile: &Profile,
    session_key: &SessionKey,
    issuer: &str,
) -> Result<systemprompt_cloud::CliSession> {
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
    hostname: &str,
    token: &SessionToken,
    context: &ContextId,
    args: &[String],
    timeout_secs: u64,
) -> Result<i32> {
    let executor = RemoteCliExecutor::new(&tenant_base_url(hostname), timeout_secs)
        .context("Failed to create HTTP client")?;
    let mut sink = StdioSink {
        stdout: io::stdout(),
        stderr: io::stderr(),
    };
    let request = RemoteCliRequest {
        token,
        context: Some(context),
        args,
    };
    Ok(executor.execute(request, &mut sink).await?)
}
