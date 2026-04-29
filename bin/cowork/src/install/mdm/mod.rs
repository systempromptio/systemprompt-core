#[cfg(target_os = "macos")]
pub(super) mod macos;
#[cfg(target_os = "windows")]
mod windows;

use crate::schedule::Os;
use std::path::Path;

const MDM_MACOS_SNIPPET_TMPL: &str = include_str!("../templates/mdm_macos_snippet.tmpl");

pub fn os_label(os: Os) -> &'static str {
    match os {
        Os::Mac => "macOS",
        Os::Windows => "Windows",
        Os::Linux => "Linux",
    }
}

pub fn apply_mdm(
    os: Os,
    binary: &Path,
    gateway: &str,
    pubkey: Option<&str>,
) -> Result<Vec<String>, String> {
    match os {
        #[cfg(target_os = "windows")]
        Os::Windows => windows::apply(binary, gateway, pubkey),
        #[cfg(not(target_os = "windows"))]
        Os::Windows => {
            let _ = (binary, gateway, pubkey);
            Err("--apply on Windows must be run from a Windows binary".into())
        },
        #[cfg(target_os = "macos")]
        Os::Mac => macos::apply(binary, gateway, pubkey),
        #[cfg(not(target_os = "macos"))]
        Os::Mac => Err("--apply on macOS must be run from a macOS binary".into()),
        Os::Linux => Err("Linux has no Anthropic-documented MDM format; set the \
                          CLAUDE_INFERENCE_* env vars in your shell profile or systemd-user unit."
            .into()),
    }
}

pub fn windows_policy_values(
    binary: &Path,
    gateway: &str,
    pubkey: Option<&str>,
) -> Vec<(&'static str, &'static str, String)> {
    let binary_str = binary.to_string_lossy().into_owned();
    let mut values: Vec<(&'static str, &'static str, String)> = vec![
        ("inferenceProvider", "REG_SZ", "gateway".into()),
        ("inferenceGatewayBaseUrl", "REG_SZ", gateway.into()),
        ("inferenceCredentialHelper", "REG_SZ", binary_str),
        (
            "inferenceCredentialHelperTtlSec",
            "REG_DWORD",
            "3600".into(),
        ),
        ("inferenceGatewayAuthScheme", "REG_SZ", "bearer".into()),
    ];
    if let Some(pk) = pubkey {
        values.push(("inferenceManifestPubkey", "REG_SZ", pk.to_string()));
    }
    values
}

pub fn snippet(os: Os, binary: &Path, gateway_url: Option<&str>) -> String {
    let binary = binary.display();
    let gateway = gateway_url.unwrap_or("https://gateway.systemprompt.io");
    match os {
        Os::Mac => MDM_MACOS_SNIPPET_TMPL
            .replace("{gateway}", gateway)
            .replace("{binary}", &binary.to_string()),
        Os::Windows => format!(
            r#"Registry key: HKLM\SOFTWARE\Policies\Claude (machine-wide; HKCU as per-user fallback)
Format: .reg — distribute via Group Policy, Intune, or any MDM that imports .reg files

Windows Registry Editor Version 5.00

[HKEY_LOCAL_MACHINE\SOFTWARE\Policies\Claude]
"inferenceProvider"="gateway"
"inferenceGatewayBaseUrl"="{gateway}"
"inferenceCredentialHelper"="{binary}"
"inferenceCredentialHelperTtlSec"=dword:00000E10
"inferenceGatewayAuthScheme"="bearer"
"#
        ),
        Os::Linux => format!(
            r#"Anthropic does not document an MDM format for Linux.
Environment-based configuration (user shell profile or systemd-user Environment=):

export CLAUDE_INFERENCE_PROVIDER=gateway
export CLAUDE_INFERENCE_GATEWAY_BASE_URL={gateway}
export CLAUDE_INFERENCE_CREDENTIAL_HELPER={binary}
export CLAUDE_INFERENCE_CREDENTIAL_HELPER_TTL_SEC=3600
export CLAUDE_INFERENCE_GATEWAY_AUTH_SCHEME=bearer
"#
        ),
    }
}
