#[cfg(target_os = "macos")]
pub(super) mod macos;
#[cfg(target_os = "windows")]
mod windows;

use crate::schedule::Os;

const MDM_MACOS_SNIPPET_TMPL: &str = include_str!("../templates/mdm_macos_snippet.tmpl");

pub(crate) const fn os_label(os: Os) -> &'static str {
    match os {
        Os::Mac => "macOS",
        Os::Windows => "Windows",
        Os::Linux => "Linux",
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub(crate) fn refresh_managed_mcp_servers() -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        return windows::refresh_managed_mcp_servers();
    }
    #[cfg(not(target_os = "windows"))]
    {
        Ok("managedMcpServers refresh skipped (non-Windows)".into())
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn write_empty_managed_mcp_servers() -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        windows::write_managed_mcp_servers_value("[]")
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
impl crate::sync::host_sync::HostSync for ClaudeDesktopMdmSync {
    fn host_id(&self) -> &'static str {
        "claude-desktop"
    }

    async fn apply(
        &self,
        _ctx: &crate::sync::host_sync::HostSyncCtx<'_>,
    ) -> Result<(), crate::sync::ApplyError> {
        match refresh_managed_mcp_servers() {
            Ok(line) => {
                tracing::info!(
                    target: "bridge::mdm",
                    written = %line,
                    "managedMcpServers policy value refreshed"
                );
                Ok(())
            },
            Err(e) => Err(crate::sync::ApplyError::Io {
                context: format!("mdm refresh: {e}"),
                source: std::io::Error::other(e),
            }),
        }
    }

    fn clear(&self) -> Result<(), crate::sync::ApplyError> {
        match write_empty_managed_mcp_servers() {
            Ok(line) => {
                tracing::info!(
                    target: "bridge::mdm",
                    written = %line,
                    "managedMcpServers policy cleared"
                );
                Ok(())
            },
            Err(e) => Err(crate::sync::ApplyError::Io {
                context: format!("mdm clear: {e}"),
                source: std::io::Error::other(e),
            }),
        }
    }
}

pub(crate) fn apply_mdm(
    os: Os,
    gateway: &str,
    pubkey: Option<&str>,
) -> Result<Vec<String>, String> {
    match os {
        #[cfg(target_os = "windows")]
        Os::Windows => windows::apply(gateway, pubkey),
        #[cfg(not(target_os = "windows"))]
        Os::Windows => {
            _ = (gateway, pubkey);
            Err("--apply on Windows must be run from a Windows binary".into())
        },
        #[cfg(target_os = "macos")]
        Os::Mac => macos::apply(gateway, pubkey),
        #[cfg(not(target_os = "macos"))]
        Os::Mac => Err("--apply on macOS must be run from a macOS binary".into()),
        Os::Linux => Err("Linux has no Anthropic-documented MDM format; set the \
                          CLAUDE_INFERENCE_* env vars in your shell profile or systemd-user unit."
            .into()),
    }
}

#[cfg(target_os = "windows")]
#[must_use]
pub(crate) fn windows_policy_values(
    _gateway: &str,
    pubkey: Option<&str>,
) -> Vec<(&'static str, &'static str, String)> {
    // Stable, MDM-deployable policy only. `inferenceGatewayBaseUrl` and
    // `inferenceGatewayApiKey` are runtime values owned by the running GUI
    // (it writes them to HKCU when it binds the live proxy port and mints
    // the loopback secret). Writing them here would pin a stale port/key
    // and collide with the GUI's HKCU values.
    let mut values: Vec<(&'static str, &'static str, String)> = vec![
        ("inferenceProvider", "REG_SZ", "gateway".into()),
        ("inferenceGatewayAuthScheme", "REG_SZ", "bearer".into()),
        ("disableEssentialTelemetry", "REG_SZ", "true".into()),
        ("disableNonessentialTelemetry", "REG_SZ", "true".into()),
        ("disableNonessentialServices", "REG_SZ", "true".into()),
        ("disableAutoUpdates", "REG_SZ", "true".into()),
        ("disableDeploymentModeChooser", "REG_SZ", "true".into()),
        // MCP-server lockdown: managed-only source, loopback-only egress. The
        // `managedMcpServers` value itself is filled in below; these two flags
        // make Cowork ignore user-added MCP servers and refuse non-loopback
        // egress so all traffic flows through the bridge proxy.
        ("isLocalDevMcpEnabled", "REG_SZ", "false".into()),
        (
            "coworkEgressAllowedHosts",
            "REG_SZ",
            r#"["127.0.0.1"]"#.into(),
        ),
    ];
    let managed = managed_mcp_servers_json().unwrap_or_else(|| "[]".to_string());
    values.push(("managedMcpServers", "REG_SZ", managed));
    if let Some(pk) = pubkey {
        values.push(("inferenceManifestPubkey", "REG_SZ", pk.to_string()));
    }
    values
}

// `"oauth": true` is the documented value for "dynamic client registration"
// in claude.com/docs/cowork/3p/configuration. Cowork's bundle schema
// (`$Qr.oauth`) is a union of `boolean` or an object with required `clientId`
// — an empty object `{}` satisfies neither and is silently dropped, which is
// why an earlier version of this code produced MCP servers that surfaced in
// the UI but never triggered the OAuth flow on Connect.
#[cfg(target_os = "windows")]
#[must_use]
pub(crate) fn managed_mcp_servers_json() -> Option<String> {
    let registry = crate::mcp_registry::snapshot();
    if registry.is_empty() {
        return Some("[]".to_string());
    }
    let mut slugs: Vec<&String> = registry.keys().collect();
    slugs.sort();
    let entries: Vec<serde_json::Value> = slugs
        .iter()
        .filter_map(|slug| {
            let upstream = registry.get(*slug)?;
            Some(serde_json::json!({
                "name": slug,
                "url": upstream.url.as_str(),
                "transport": "http",
                "oauth": true,
            }))
        })
        .collect();
    serde_json::to_string(&entries).ok()
}

#[expect(
    clippy::literal_string_with_formatting_args,
    reason = "{gateway} is a template placeholder consumed by str::replace, not a fmt arg"
)]
pub(crate) fn snippet(os: Os, gateway_url: Option<&str>) -> String {
    let gateway = gateway_url.unwrap_or("https://gateway.systemprompt.io");
    match os {
        Os::Mac => MDM_MACOS_SNIPPET_TMPL.replace("{gateway}", gateway),
        Os::Windows => {
            r#"Registry key: HKLM\SOFTWARE\Policies\Claude (machine-wide; HKCU as per-user fallback)
Format: .reg — distribute via Group Policy, Intune, or any MDM that imports .reg files

Windows Registry Editor Version 5.00

[HKEY_LOCAL_MACHINE\SOFTWARE\Policies\Claude]
"inferenceProvider"="gateway"
"inferenceGatewayAuthScheme"="bearer"
"disableEssentialTelemetry"="true"
"disableNonessentialTelemetry"="true"
"disableNonessentialServices"="true"
"disableAutoUpdates"="true"
"disableDeploymentModeChooser"="true"
; inferenceGatewayBaseUrl and inferenceGatewayApiKey are written to HKCU by the
; running Bridge GUI when it binds the loopback proxy. Do not pin them here.
"#
            .to_string()
        },
        Os::Linux => format!(
            r"Anthropic does not document an MDM format for Linux.
Environment-based configuration (user shell profile or systemd-user Environment=):

export CLAUDE_INFERENCE_PROVIDER=gateway
export CLAUDE_INFERENCE_GATEWAY_BASE_URL={gateway}
export CLAUDE_INFERENCE_GATEWAY_API_KEY=<loopback-secret-from-$XDG_CONFIG_HOME/systemprompt/bridge-loopback.key>
export CLAUDE_INFERENCE_GATEWAY_AUTH_SCHEME=bearer
"
        ),
    }
}
