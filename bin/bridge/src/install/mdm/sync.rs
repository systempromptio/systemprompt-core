//! The Claude Desktop MDM emitter: refreshes the `managedMcpServers` policy on
//! every sync so Cowork's connectors follow the manifest.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[cfg(target_os = "macos")]
fn refresh_managed_mcp_servers(
    mcp: &super::MdmPayloadInputs<'_>,
) -> Result<String, super::MdmError> {
    let base_url = mcp.loopback.origin();
    super::macos::apply(mcp, &base_url, None).map(|report| report.lines.join("; "))
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn enforce_managed_policy(mcp: &super::MdmPayloadInputs<'_>) -> Result<String, super::MdmError> {
    #[cfg(target_os = "windows")]
    {
        super::windows::enforce_managed_policy(mcp)
    }
    #[cfg(not(target_os = "windows"))]
    {
        refresh_managed_mcp_servers(mcp)
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn write_empty_managed_mcp_servers(
    mcp: &super::MdmPayloadInputs<'_>,
) -> Result<String, super::MdmError> {
    #[cfg(target_os = "windows")]
    {
        super::windows::write_managed_mcp_servers_value(mcp.policy_store, "[]")
    }
    #[cfg(not(target_os = "windows"))]
    {
        refresh_managed_mcp_servers(mcp)
    }
}

// Why: Desktop's `toolPolicy` names tools one by one, so the policy write
// needs each server's current tool list before it runs.
#[cfg(any(target_os = "macos", target_os = "windows"))]
async fn refresh_tool_catalog(ctx: &crate::host_sync::HostSyncCtx<'_>) {
    if ctx.mcp_registry.is_empty() {
        return;
    }
    let results = crate::proxy::mcp_probe::probe_all(ctx.loopback, ctx.mcp_registry).await;
    let slugs: Vec<String> = ctx.mcp_registry.keys().cloned().collect();
    let outcome =
        super::tool_catalog::record(&results).and_then(|_| super::tool_catalog::retain(&slugs));
    match outcome {
        Ok(()) => tracing::info!(
            target: "bridge::mdm",
            servers = results
                .iter()
                .filter(|r| r.state == crate::proxy::mcp_probe::McpAuthState::Authenticated)
                .count(),
            "mcp tool catalog refreshed for the desktop tool policy"
        ),
        Err(e) => {
            tracing::warn!(
                target: "bridge::mdm",
                error = %e,
                "mcp tool catalog not written; desktop tool policy keeps its last names"
            );
            ctx.warnings.push(
                "claude-desktop",
                format!("tool catalog not updated ({e}); the tool policy keeps its last names"),
            );
        },
    }
}

#[cfg(target_os = "windows")]
fn elevation_needed(e: &super::MdmError) -> Option<String> {
    use crate::config::store::ConfigStoreError;
    let mut store = match e {
        super::MdmError::Partial { source, .. } => match source.as_ref() {
            super::MdmError::Store(store) => store,
            _ => return None,
        },
        super::MdmError::Store(store) => store,
        _ => return None,
    };
    while let ConfigStoreError::Partial { source, .. } = store {
        store = source;
    }
    match store {
        ConfigStoreError::HiveConflict { subkey, differing } => Some(format!(
            "HKLM\\{subkey} holds different values for {}",
            differing.join(", ")
        )),
        ConfigStoreError::AccessDenied { hive, subkey } => {
            Some(format!("writing {subkey} under {hive} was denied"))
        },
        _ => None,
    }
}

// Why: sync runs unattended, so a machine policy only an administrator can
// replace is reported as such and left for a user-triggered repair; a UAC
// prompt must never appear without a gesture.
#[cfg(any(target_os = "macos", target_os = "windows"))]
fn classify_refresh_error(e: super::MdmError) -> crate::host_sync::ApplyError {
    #[cfg(target_os = "windows")]
    if let Some(detail) = elevation_needed(&e)
        && !crate::winproc::is_elevated()
    {
        return crate::host_sync::ApplyError::ElevationRequired {
            what: "the Claude Desktop machine policy",
            detail,
        };
    }
    crate::host_sync::ApplyError::Io {
        context: format!("mdm refresh: {e}"),
        source: std::io::Error::other(e),
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub(crate) struct ClaudeDesktopMdmSync;

#[cfg(any(target_os = "macos", target_os = "windows"))]
#[async_trait::async_trait]
impl crate::host_sync::HostSync for ClaudeDesktopMdmSync {
    fn host_id(&self) -> &'static str {
        "claude-desktop"
    }

    async fn apply(
        &self,
        ctx: &crate::host_sync::HostSyncCtx<'_>,
    ) -> Result<(), crate::host_sync::ApplyError> {
        refresh_tool_catalog(ctx).await;
        match enforce_managed_policy(&super::MdmPayloadInputs {
            policy_store: ctx.policy_store,
            loopback: ctx.loopback,
            registry: ctx.mcp_registry,
            egress_allowed_hosts: None,
        }) {
            Ok(line) => {
                tracing::info!(
                    target: "bridge::mdm",
                    written = %line,
                    "managed policy enforced on sync"
                );
                Ok(())
            },
            Err(e) => Err(classify_refresh_error(e)),
        }
    }

    fn clear(
        &self,
        ctx: &crate::host_sync::HostSyncCtx<'_>,
    ) -> Result<(), crate::host_sync::ApplyError> {
        let empty = crate::mcp_registry::McpRegistry::new();
        match write_empty_managed_mcp_servers(&super::MdmPayloadInputs {
            policy_store: ctx.policy_store,
            loopback: ctx.loopback,
            registry: &empty,
            egress_allowed_hosts: None,
        }) {
            Ok(line) => {
                tracing::info!(
                    target: "bridge::mdm",
                    written = %line,
                    "managedMcpServers policy cleared"
                );
                Ok(())
            },
            Err(e) => Err(crate::host_sync::ApplyError::Io {
                context: format!("mdm clear: {e}"),
                source: std::io::Error::other(e),
            }),
        }
    }
}

crate::register_host_sync!(ClaudeDesktopMdmSync);
