//! MDM deployment snippets and managed MCP-server refresh.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub(crate) mod egress;
mod error;
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub(crate) mod linux;
#[cfg(target_os = "macos")]
pub(super) mod macos;
#[cfg(target_os = "windows")]
mod windows;

pub use egress::{cowork_egress_allowed_hosts, parse_egress_allowed_hosts};
pub use error::MdmError;

use crate::schedule::Os;

const MDM_MACOS_SNIPPET_TMPL: &str = include_str!("../templates/mdm_macos_snippet.tmpl");

#[must_use]
pub fn is_uuid_like(s: &str) -> bool {
    s.len() == 36
        && s.bytes().filter(|&b| b == b'-').count() == 4
        && uuid::Uuid::try_parse(s).is_ok()
}

pub(crate) const fn os_label(os: Os) -> &'static str {
    match os {
        Os::Mac => "macOS",
        Os::Windows => "Windows",
        Os::Linux => "Linux",
    }
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(
    not(any(target_os = "macos", target_os = "windows")),
    expect(
        dead_code,
        reason = "only the macOS and Windows MDM payloads embed the managed-MCP servers"
    )
)]
pub struct MdmPayloadInputs<'a> {
    pub loopback: &'a crate::proxy::LoopbackEndpoint,
    pub registry: &'a crate::mcp_registry::McpRegistry,
    pub egress_allowed_hosts: Option<&'a [String]>,
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
#[cfg_attr(
    target_os = "macos",
    expect(
        clippy::unnecessary_wraps,
        reason = "only the Windows branch is fallible; the signature stays uniform so callers need no cfg"
    )
)]
pub(crate) fn refresh_managed_mcp_servers(mcp: &MdmPayloadInputs<'_>) -> Result<String, MdmError> {
    #[cfg(target_os = "windows")]
    {
        windows::refresh_managed_mcp_servers(mcp)
    }
    #[cfg(not(target_os = "windows"))]
    {
        _ = mcp;
        Ok("managedMcpServers refresh skipped (non-Windows)".into())
    }
}

#[cfg(target_os = "windows")]
pub(crate) fn remove_windows_policy() -> Result<bool, MdmError> {
    windows::remove_policy()
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
#[cfg_attr(
    target_os = "macos",
    expect(
        clippy::unnecessary_wraps,
        reason = "only the Windows branch is fallible; the signature stays uniform so callers need no cfg"
    )
)]
fn write_empty_managed_mcp_servers() -> Result<String, MdmError> {
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
impl crate::host_sync::HostSync for ClaudeDesktopMdmSync {
    fn host_id(&self) -> &'static str {
        "claude-desktop"
    }

    async fn apply(
        &self,
        ctx: &crate::host_sync::HostSyncCtx<'_>,
    ) -> Result<(), crate::host_sync::ApplyError> {
        match refresh_managed_mcp_servers(&MdmPayloadInputs {
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

pub(crate) fn apply_mdm(
    os: Os,
    mcp: &MdmPayloadInputs<'_>,
    gateway: &str,
    pubkey: Option<&str>,
) -> Result<Vec<String>, MdmError> {
    // Why: the Linux snippet embeds neither the loopback endpoint nor the
    // egress allowlist; Windows carries MCP through `refresh_managed_mcp_servers`.
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let _ = mcp;
    match os {
        #[cfg(target_os = "windows")]
        Os::Windows => windows::apply(mcp, gateway, pubkey),
        #[cfg(not(target_os = "windows"))]
        Os::Windows => {
            _ = (gateway, pubkey);
            Err(MdmError::WrongHostOs { os: "Windows" })
        },
        #[cfg(target_os = "macos")]
        Os::Mac => macos::apply(mcp, gateway, pubkey),
        #[cfg(not(target_os = "macos"))]
        Os::Mac => Err(MdmError::WrongHostOs { os: "macOS" }),
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        Os::Linux => linux::apply(gateway),
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        Os::Linux => Err(MdmError::WrongHostOs { os: "Linux" }),
    }
}

#[cfg(target_os = "windows")]
#[must_use]
pub fn windows_policy_values(
    _gateway: &str,
    pubkey: Option<&str>,
    org_uuid: Option<&str>,
    egress_allowed_hosts: Option<&[String]>,
) -> Vec<(&'static str, &'static str, String)> {
    let mut values: Vec<(&'static str, &'static str, String)> = vec![
        ("inferenceProvider", "REG_SZ", "gateway".into()),
        ("inferenceGatewayAuthScheme", "REG_SZ", "bearer".into()),
        ("disableEssentialTelemetry", "REG_SZ", "true".into()),
        ("disableNonessentialTelemetry", "REG_SZ", "true".into()),
        ("disableNonessentialServices", "REG_SZ", "true".into()),
        ("disableAutoUpdates", "REG_SZ", "true".into()),
        ("disableDeploymentModeChooser", "REG_SZ", "true".into()),
        ("isLocalDevMcpEnabled", "REG_SZ", "false".into()),
    ];
    // Why: omitted by default so Cowork keeps its own unrestricted egress. A
    // pinned allowlist here left agents with no internet at all; it is now an
    // explicit opt-in for regulated deployments.
    if let Some(hosts) = cowork_egress_allowed_hosts(egress_allowed_hosts) {
        values.push((
            "coworkEgressAllowedHosts",
            "REG_SZ",
            egress::windows_policy_value(&hosts),
        ));
    }
    // Why: without a pre-trusted workspace Cowork falls back to protected host
    // paths and blocks on `request_cowork_directory`; `isDefaultSelected` skips
    // the trust prompt.
    let workspace = crate::brand::brand().workspace_dir_name;
    if !workspace.is_empty() {
        let json =
            serde_json::json!([{ "path": format!("~/{workspace}"), "isDefaultSelected": true }]);
        values.push(("allowedWorkspaceFolders", "REG_SZ", json.to_string()));
    }
    if let Some(pk) = pubkey {
        values.push(("inferenceManifestPubkey", "REG_SZ", pk.to_owned()));
    }
    if let Some(uuid) = org_uuid.filter(|u| is_uuid_like(u)) {
        values.push(("deploymentOrganizationUuid", "REG_SZ", uuid.to_owned()));
    }
    values
}

// Why: Cowork's OAuth flow rejects the gateway's non-HTTPS authorize URL, so
// servers must point at the loopback proxy that injects the gateway JWT.
#[cfg(target_os = "windows")]
#[must_use]
pub(crate) fn managed_mcp_servers_json(mcp: &MdmPayloadInputs<'_>) -> Option<String> {
    let MdmPayloadInputs {
        loopback, registry, ..
    } = *mcp;
    if registry.is_empty() {
        return Some("[]".to_owned());
    }
    let bearer = match loopback.bearer() {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!(
                target: "bridge::install::mdm",
                error = %e,
                "loopback secret unavailable; emitting empty managed MCP server list"
            );
            return None;
        },
    };
    let mut slugs: Vec<&String> = registry.keys().collect();
    slugs.sort();
    let entries: Vec<serde_json::Value> = slugs
        .iter()
        .map(|slug| {
            serde_json::json!({
                "name": slug,
                "url": loopback.mcp_url(slug.as_str()),
                "transport": "http",
                "headers": { "Authorization": bearer.clone() },
            })
        })
        .collect();
    serde_json::to_string(&entries).ok()
}

#[expect(
    clippy::literal_string_with_formatting_args,
    reason = "{gateway} is a template placeholder consumed by str::replace, not a fmt arg"
)]
pub fn snippet(os: Os, gateway_url: Option<&str>) -> String {
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
"isLocalDevMcpEnabled"="false"
"allowedWorkspaceFolders"="[{\"path\":\"~/{workspace}\",\"isDefaultSelected\":true}]"
; Optional: restrict which hosts Cowork may reach. Omit for unrestricted egress
; (the default). Loopback-only is the air-gapped/regulated posture; apply it with
; `install --apply --egress-allowed-hosts loopback` so the Bridge keeps the value
; in step with this key.
; "coworkEgressAllowedHosts"="[\"127.0.0.1\"]"
; Optional: identify this deployment to your org for telemetry/support.
; Omit to use Anthropic's shared placeholder UUID. Standard hyphenated form only.
; "deploymentOrganizationUuid"="xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"
; inferenceGatewayBaseUrl and inferenceGatewayApiKey are written into this policy
; key by the Bridge when you apply the Claude Desktop host profile, and re-applied
; whenever the local loopback secret rotates. Do not pin them here.
"#
            .replace("{workspace}", crate::brand::brand().workspace_dir_name)
        },
        Os::Linux => {
            let config_dir = crate::brand::brand().config_dir;
            let bin = crate::brand::brand().binary_name;
            format!(
                r"Anthropic does not document an MDM format for Linux, so `{bin} install --apply`
writes the equivalent environment instead:

  $XDG_CONFIG_HOME/{config_dir}/env.sh   exports the two variables below
  ~/.profile                             a managed block sourcing that file

export ANTHROPIC_BASE_URL={gateway}
export ANTHROPIC_AUTH_TOKEN=$(cat $XDG_CONFIG_HOME/{config_dir}/bridge-loopback.key)

The token is read from the key file at eval time, so a rotated loopback secret
needs no rewrite. Rerun with --apply to write these directly.
"
            )
        },
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
crate::register_host_sync!(ClaudeDesktopMdmSync);
