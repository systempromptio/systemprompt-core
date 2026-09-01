//! The Claude Desktop MDM emitter: refreshes the `managedMcpServers` policy on
//! every sync so Cowork's connectors follow the manifest.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[cfg(any(target_os = "macos", target_os = "windows"))]
#[cfg_attr(
    target_os = "macos",
    expect(
        clippy::unnecessary_wraps,
        reason = "only the Windows branch is fallible; the signature stays uniform so callers need no cfg"
    )
)]
pub(crate) fn refresh_managed_mcp_servers(
    mcp: &super::MdmPayloadInputs<'_>,
) -> Result<String, super::MdmError> {
    #[cfg(target_os = "windows")]
    {
        super::windows::refresh_managed_mcp_servers(mcp)
    }
    #[cfg(not(target_os = "windows"))]
    {
        _ = mcp;
        Ok("managedMcpServers refresh skipped (non-Windows)".into())
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
#[cfg_attr(
    target_os = "macos",
    expect(
        clippy::unnecessary_wraps,
        reason = "only the Windows branch is fallible; the signature stays uniform so callers need no cfg"
    )
)]
fn write_empty_managed_mcp_servers() -> Result<String, super::MdmError> {
    #[cfg(target_os = "windows")]
    {
        super::windows::write_managed_mcp_servers_value("[]")
    }
    #[cfg(not(target_os = "windows"))]
    {
        Ok("managedMcpServers clear skipped (non-Windows)".into())
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
        match refresh_managed_mcp_servers(&super::MdmPayloadInputs {
            loopback: ctx.loopback,
            registry: ctx.mcp_registry,
            egress_allowed_hosts: None,
        }) {
            Ok(line) => {
                tracing::info!(
                    target: "bridge::mdm",
                    written = %line,
                    "managedMcpServers policy value refreshed"
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
        _ctx: &crate::host_sync::HostSyncCtx<'_>,
    ) -> Result<(), crate::host_sync::ApplyError> {
        match write_empty_managed_mcp_servers() {
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
