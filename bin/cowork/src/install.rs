use crate::config;
use crate::http::GatewayClient;
use crate::output::diag;
use crate::paths::{self, OrgPluginsLocation, Scope};
use crate::schedule::{self, Os};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

pub struct InstallOptions {
    pub print_mdm: Option<Os>,
    pub emit_schedule_template: Option<Os>,
    pub gateway_url: Option<String>,
    pub no_pubkey_fetch: bool,
    pub apply: bool,
}

pub fn install(opts: InstallOptions) -> ExitCode {
    let binary = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            diag(&format!("cannot determine current executable path: {e}"));
            return ExitCode::from(1);
        },
    };

    let location = match paths::org_plugins_effective() {
        Some(l) => l,
        None => {
            diag("cannot resolve org-plugins directory for this OS");
            return ExitCode::from(1);
        },
    };

    if let Err(e) = bootstrap_directory(&location) {
        diag(&format!("directory bootstrap failed: {e}"));
        return ExitCode::from(1);
    }

    if let Err(e) = write_version_sentinel(&location.path, &binary, opts.gateway_url.as_deref()) {
        diag(&format!("version sentinel write failed: {e}"));
        return ExitCode::from(1);
    }

    if let Some(ref url) = opts.gateway_url {
        if let Err(e) = config::ensure_gateway_url(url) {
            diag(&format!("warning: could not persist gateway_url to config: {e}"));
        }
    }

    if !opts.no_pubkey_fetch {
        if let Some(ref url) = opts.gateway_url {
            match GatewayClient::new(url.clone()).fetch_pubkey() {
                Ok(pubkey) => {
                    if let Err(e) = config::persist_pinned_pubkey(&pubkey) {
                        diag(&format!(
                            "warning: failed to pin manifest pubkey (continuing): {e}"
                        ));
                    } else {
                        eprintln!("Pinned manifest signing pubkey from {url}");
                    }
                },
                Err(e) => {
                    diag(&format!(
                        "warning: pubkey fetch failed (you can rerun install later): {e}"
                    ));
                },
            }
        }
    }

    println!("Installed systemprompt-cowork integration");
    println!("  org-plugins: {} ({})", location.path.display(), match location.scope {
        Scope::System => "system-wide",
        Scope::User => "per-user",
    });
    println!("  metadata:    {}", paths::metadata_dir(&location.path).display());
    println!("  binary:      {}", binary.display());

    let target_os = opts.print_mdm.unwrap_or_else(Os::current);
    let gateway_for_mdm = opts
        .gateway_url
        .clone()
        .or_else(|| config::load().gateway_url)
        .unwrap_or_else(|| "https://gateway.systemprompt.io".into());

    if opts.apply {
        match apply_mdm(target_os, &binary, &gateway_for_mdm) {
            Ok(summary) => {
                println!();
                println!("--- MDM applied ({}) ---", os_label(target_os));
                for line in summary {
                    println!("  {line}");
                }
            },
            Err(e) => {
                diag(&format!("apply failed: {e}"));
                return ExitCode::from(1);
            },
        }
    } else {
        println!();
        println!("--- MDM configuration ({}) ---", os_label(target_os));
        println!("{}", mdm_snippet(target_os, &binary, Some(&gateway_for_mdm)));
        println!("Tip: rerun with --apply to write these keys directly.");
    }

    if let Some(schedule_os) = opts.emit_schedule_template {
        let filename = schedule::template_filename(schedule_os);
        let content = schedule::template(schedule_os, &binary);
        let out = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(filename);
        if let Err(e) = fs::write(&out, content) {
            diag(&format!("failed to write {}: {e}", out.display()));
            return ExitCode::from(1);
        }
        println!();
        println!("--- Schedule template ({}) ---", os_label(schedule_os));
        println!("wrote: {}", out.display());
        println!("{}", schedule::install_hint(schedule_os));
    }

    ExitCode::SUCCESS
}

pub fn uninstall(purge: bool) -> ExitCode {
    let location = match paths::org_plugins_effective() {
        Some(l) => l,
        None => {
            diag("cannot resolve org-plugins directory for this OS");
            return ExitCode::from(1);
        },
    };

    let metadata = paths::metadata_dir(&location.path);
    if metadata.exists() {
        if let Err(e) = fs::remove_dir_all(&metadata) {
            diag(&format!(
                "failed to remove metadata dir {}: {e}",
                metadata.display()
            ));
            return ExitCode::from(1);
        }
        println!("Removed {}", metadata.display());
    } else {
        println!("No metadata dir at {} (already clean)", metadata.display());
    }

    let staging = paths::staging_dir(&location.path);
    if staging.exists() {
        let _ = fs::remove_dir_all(&staging);
    }

    if purge {
        match crate::setup::logout() {
            Ok(p) => println!("Purged credentials: {}", p.pat_file.display()),
            Err(e) => diag(&format!("credential purge failed: {e}")),
        }
    } else {
        println!("Credentials left intact. Use `systemprompt-cowork uninstall --purge` to also clear them.");
    }
    ExitCode::SUCCESS
}

fn bootstrap_directory(loc: &OrgPluginsLocation) -> std::io::Result<()> {
    fs::create_dir_all(&loc.path)?;
    let meta = paths::metadata_dir(&loc.path);
    fs::create_dir_all(&meta)?;
    Ok(())
}

fn write_version_sentinel(
    org_plugins: &Path,
    binary: &Path,
    gateway_url: Option<&str>,
) -> std::io::Result<()> {
    let sentinel = paths::metadata_dir(org_plugins).join(paths::VERSION_SENTINEL);
    let payload = serde_json::json!({
        "binary": binary.display().to_string(),
        "binary_version": env!("CARGO_PKG_VERSION"),
        "installed_at": current_iso8601(),
        "gateway_url": gateway_url,
    });
    fs::write(&sentinel, serde_json::to_vec_pretty(&payload).unwrap_or_default())?;
    Ok(())
}

fn current_iso8601() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "unknown".into())
}

fn apply_mdm(os: Os, binary: &Path, gateway: &str) -> Result<Vec<String>, String> {
    match os {
        Os::Windows => apply_windows(binary, gateway),
        Os::MacOs => apply_macos(binary, gateway),
        Os::Linux => Err(
            "Linux has no Anthropic-documented MDM format; \
            set the CLAUDE_INFERENCE_* env vars in your shell profile or systemd-user unit."
                .into(),
        ),
    }
}

#[cfg(target_os = "windows")]
fn apply_windows(binary: &Path, gateway: &str) -> Result<Vec<String>, String> {
    use std::process::Command;
    let binary_str = binary.to_string_lossy();
    let key = r"HKCU\SOFTWARE\Policies\Claude";
    let values: &[(&str, &str, String)] = &[
        ("inferenceProvider", "REG_SZ", "gateway".into()),
        ("inferenceGatewayBaseUrl", "REG_SZ", gateway.into()),
        ("inferenceCredentialHelper", "REG_SZ", binary_str.into_owned()),
        ("inferenceCredentialHelperTtlSec", "REG_DWORD", "3600".into()),
        ("inferenceGatewayAuthScheme", "REG_SZ", "bearer".into()),
    ];
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
    summary.push(
        "Fully quit Cowork (tray icon → Quit) and relaunch to pick up new policy.".into(),
    );
    Ok(summary)
}

#[cfg(not(target_os = "windows"))]
fn apply_windows(_binary: &Path, _gateway: &str) -> Result<Vec<String>, String> {
    Err("--apply on Windows must be run from a Windows binary".into())
}

#[cfg(target_os = "macos")]
fn apply_macos(binary: &Path, gateway: &str) -> Result<Vec<String>, String> {
    use std::process::Command;
    let domain = "com.anthropic.claudefordesktop";
    let binary_str = binary.to_string_lossy();
    let values: &[(&str, &str, String)] = &[
        ("inferenceProvider", "-string", "gateway".into()),
        ("inferenceGatewayBaseUrl", "-string", gateway.into()),
        ("inferenceCredentialHelper", "-string", binary_str.into_owned()),
        ("inferenceCredentialHelperTtlSec", "-int", "3600".into()),
        ("inferenceGatewayAuthScheme", "-string", "bearer".into()),
    ];
    let mut summary = Vec::with_capacity(values.len() + 2);
    summary.push(format!("defaults domain: {domain}"));
    for (name, kind, data) in values {
        let status = Command::new("defaults")
            .args(["write", domain, name, kind, data])
            .status()
            .map_err(|e| format!("defaults write {name}: {e}"))?;
        if !status.success() {
            return Err(format!(
                "defaults write {name} exited with {}",
                status.code().unwrap_or(-1)
            ));
        }
        summary.push(format!("wrote {name} ({kind})"));
    }
    summary.push(
        "Note: `defaults write` sets per-user preferences. For managed-fleet deployment, export a .mobileconfig and distribute via MDM.".into(),
    );
    summary.push(
        "Fully quit Cowork (Cmd+Q) and relaunch to pick up new preferences.".into(),
    );
    Ok(summary)
}

#[cfg(not(target_os = "macos"))]
fn apply_macos(_binary: &Path, _gateway: &str) -> Result<Vec<String>, String> {
    Err("--apply on macOS must be run from a macOS binary".into())
}

fn os_label(os: Os) -> &'static str {
    match os {
        Os::MacOs => "macOS",
        Os::Windows => "Windows",
        Os::Linux => "Linux",
    }
}

fn mdm_snippet(os: Os, binary: &Path, gateway_url: Option<&str>) -> String {
    let binary = binary.display();
    let gateway = gateway_url.unwrap_or("https://gateway.systemprompt.io");
    match os {
        Os::MacOs => format!(
            r#"Domain: com.anthropic.claudefordesktop
Format: .mobileconfig (managed preference)

<dict>
  <key>inferenceProvider</key>
  <string>gateway</string>
  <key>inferenceGatewayBaseUrl</key>
  <string>{gateway}</string>
  <key>inferenceCredentialHelper</key>
  <string>{binary}</string>
  <key>inferenceCredentialHelperTtlSec</key>
  <integer>3600</integer>
  <key>inferenceGatewayAuthScheme</key>
  <string>bearer</string>
</dict>
"#
        ),
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
