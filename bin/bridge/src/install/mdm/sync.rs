//! The Claude Desktop MDM emitter: refreshes the `managedMcpServers` policy on
//! every sync so Cowork's connectors follow the manifest.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

// Why: this used to be a no-op that returned "written by install --apply", so
// a manifest that gained or lost an MCP server never reached the plist until
// somebody re-ran that command by hand — connectors simply did not sync on
// macOS. Re-rendering the managed preferences is the macOS equivalent of the
// Windows registry re-assert. `macos::apply` compares the rendered bytes with
// what is on disk and elevates only when they differ, so an unchanged manifest
// costs one file read and raises no prompt.
#[cfg(target_os = "macos")]
fn refresh_managed_mcp_servers(
    mcp: &super::MdmPayloadInputs<'_>,
) -> Result<String, super::MdmError> {
    let base_url = mcp.loopback.origin();
    super::macos::apply(mcp, &base_url, None).map(|_| {
        format!(
            "managedMcpServers refreshed ({} servers)",
            mcp.registry.len()
        )
    })
}

// Why: both platforms re-assert the whole policy on sync so a drifted key
// self-heals.
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
        _ = mcp;
        super::windows::write_managed_mcp_servers_value("[]")
    }
    #[cfg(not(target_os = "windows"))]
    {
        refresh_managed_mcp_servers(mcp)
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
        match enforce_managed_policy(&super::MdmPayloadInputs {
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
            Err(e) => Err(crate::host_sync::ApplyError::Io {
                context: format!("mdm refresh: {e}"),
                source: std::io::Error::other(e),
            }),
        }
    }

    fn clear(
        &self,
        ctx: &crate::host_sync::HostSyncCtx<'_>,
    ) -> Result<(), crate::host_sync::ApplyError> {
        // Why: clearing means publishing no servers, so the payload is
        // rendered from an empty registry rather than the context's.
        let empty = crate::mcp_registry::McpRegistry::new();
        match write_empty_managed_mcp_servers(&super::MdmPayloadInputs {
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
