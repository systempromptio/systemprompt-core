use crate::schedule::Os;
use std::path::Path;

const MDM_MACOS_SNIPPET_TMPL: &str = include_str!("templates/mdm_macos_snippet.tmpl");

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
        Os::Windows => apply_windows(binary, gateway, pubkey),
        Os::Mac => super::macos::apply(binary, gateway, pubkey),
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

#[cfg(target_os = "windows")]
fn apply_windows(
    binary: &Path,
    gateway: &str,
    pubkey: Option<&str>,
) -> Result<Vec<String>, String> {
    use std::process::Command;
    let key = r"HKCU\SOFTWARE\Policies\Claude";
    let values = windows_policy_values(binary, gateway, pubkey);
    let mut summary = Vec::with_capacity(values.len() + 1);
    summary.push(format!("registry key: {key}"));
    for (name, kind, data) in values {
        let status = Command::new("reg")
            .args(["add", key, "/v", name, "/t", kind, "/d", data, "/f"])
            .status()
            .map_err(|e| format!("reg add {name}: {e}"))?;
        if !status.success() {
            return Err(format!(
                "reg add {name} exited with {}",
                status.code().unwrap_or(-1)
            ));
        }
        summary.push(format!("wrote {name} ({kind})"));
    }
    if gateway.starts_with("http://") && !gateway.contains("://127.0.0.1") {
        summary.push(
            "warning: Cowork rejects http:// for non-127.0.0.1 hosts. Re-run --apply with http://127.0.0.1:<port> or switch to https://.".into(),
        );
    }
    summary.push("Fully quit Cowork (tray icon → Quit) and relaunch to pick up new policy.".into());
    Ok(summary)
}

#[cfg(not(target_os = "windows"))]
fn apply_windows(
    _binary: &Path,
    _gateway: &str,
    _pubkey: Option<&str>,
) -> Result<Vec<String>, String> {
    Err("--apply on Windows must be run from a Windows binary".into())
}

pub fn snippet(os: Os, binary: &Path, gateway_url: Option<&str>) -> String {
    let binary = binary.display();
    let gateway = gateway_url.unwrap_or("https://gateway.systemprompt.io");
    match os {
        Os::Mac => MDM_MACOS_SNIPPET_TMPL
            .replace("{gateway}", gateway)
            .replace("{binary}", &binary.to_string()),
        Os::Windows => format!(
            r#"Registry key: HKCU\SOFTWARE\Policies\Claude
Format: .reg

Windows Registry Editor Version 5.00

[HKEY_CURRENT_USER\SOFTWARE\Policies\Claude]
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
