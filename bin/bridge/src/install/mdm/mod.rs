//! MDM deployment snippets and managed MCP-server refresh.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub(crate) mod egress;
mod error;
mod inference;
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub(crate) mod linux;
#[cfg(target_os = "macos")]
pub(super) mod macos;
#[cfg(target_os = "macos")]
mod macos_payload;
pub mod policy;
#[cfg(any(target_os = "macos", target_os = "windows"))]
mod sync;
#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
mod windows_policy;

pub use egress::{cowork_egress_allowed_hosts, parse_egress_allowed_hosts};
pub use error::MdmError;
pub use inference::default_inference_models;

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
pub struct MdmPayloadInputs<'a> {
    pub loopback: &'a crate::proxy::LoopbackEndpoint,
    pub registry: &'a crate::mcp_registry::McpRegistry,
    pub egress_allowed_hosts: Option<&'a [String]>,
}

#[cfg(target_os = "windows")]
pub(crate) fn remove_windows_policy() -> Result<bool, MdmError> {
    windows::remove_policy()
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

// Why: Claude's hive is Claude's; the value Cowork does not know is the
// bridge's own supply-chain pin, so it is written under the brand's key.
#[must_use]
pub fn bridge_policy_values(pubkey: Option<&str>) -> Vec<(&'static str, &'static str, String)> {
    pubkey
        .map(|pk| {
            vec![(
                crate::config::store::MANIFEST_PUBKEY_KEY,
                "REG_SZ",
                pk.to_owned(),
            )]
        })
        .unwrap_or_default()
}

pub use crate::config::store::LEGACY_MANIFEST_PUBKEY_KEY as LEGACY_PUBKEY_KEY;

#[expect(
    clippy::literal_string_with_formatting_args,
    reason = "{gateway} is a template placeholder consumed by str::replace, not a fmt arg"
)]
pub fn snippet(os: Os, gateway_url: Option<&str>) -> String {
    // Why: the fallback has to be the gateway the bridge would actually use, so
    // an admin never pastes a host this build never talks to -- and a
    // white-label prints its own gateway rather than systemprompt's.
    let gateway = gateway_url.unwrap_or_else(|| crate::brand::brand().default_gateway_url);
    match os {
        Os::Mac => MDM_MACOS_SNIPPET_TMPL
            .replace("{gateway}", gateway)
            .replace("{config_dir}", crate::brand::brand().config_dir),
        Os::Windows => {
            r#"Registry key: HKLM\SOFTWARE\Policies\Claude (machine-wide; HKCU as per-user fallback)
Format: .reg — distribute via Group Policy, Intune, or any MDM that imports .reg files

Windows Registry Editor Version 5.00

[HKEY_LOCAL_MACHINE\SOFTWARE\Policies\Claude]
"disableEssentialTelemetry"="true"
"disableNonessentialTelemetry"="true"
"disableNonessentialServices"="false"
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
; The bridge's own manifest signing key lives under the brand's key, not Claude's
; (Claude Desktop ignores keys it does not know and logs a warning for them):
; [HKEY_LOCAL_MACHINE\SOFTWARE\Policies\{config_dir}]
; "manifestPubkey"="<base64 ed25519 pubkey>"
; inferenceProvider, inferenceGatewayBaseUrl, inferenceGatewayApiKey,
; inferenceGatewayAuthScheme and inferenceModels are written into this policy key
; as one block by `install --apply` and re-asserted on every Bridge sync, so a
; rotated loopback secret self-heals. Do not pin them here: a gateway provider
; with no base URL makes Cowork refuse to start any task.
"#
            .replace("{workspace}", crate::brand::brand().workspace_dir_name)
            .replace("{config_dir}", crate::brand::brand().config_dir)
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
