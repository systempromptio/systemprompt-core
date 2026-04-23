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
    pub apply_mobileconfig: bool,
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
    let meta = paths::metadata_dir(&location.path);
    println!("  metadata:    {}", meta.display());
    println!("    user.json:    {}", meta.join(paths::USER_FRAGMENT).display());
    println!("    skills/:      {}", meta.join(paths::SKILLS_DIR).display());
    println!("    agents/:      {}", meta.join(paths::AGENTS_DIR).display());
    println!("    managed-mcp:  {}", meta.join(paths::MANAGED_MCP_FRAGMENT).display());
    println!("  binary:      {}", binary.display());
    println!("  Run `systemprompt-cowork sync` to populate user identity, skills, agents, and MCP servers.");

    let target_os = opts.print_mdm.unwrap_or_else(Os::current);
    let gateway_for_mdm = opts
        .gateway_url
        .clone()
        .or_else(|| config::load().gateway_url)
        .unwrap_or_else(|| "https://gateway.systemprompt.io".into());

    if opts.apply_mobileconfig {
        #[cfg(target_os = "macos")]
        match apply_macos_mobileconfig(&binary, &gateway_for_mdm) {
            Ok(summary) => {
                println!();
                println!("--- mobileconfig applied (macOS) ---");
                for line in summary {
                    println!("  {line}");
                }
            },
            Err(e) => {
                diag(&format!("apply --mobileconfig failed: {e}"));
                return ExitCode::from(1);
            },
        }
        #[cfg(not(target_os = "macos"))]
        {
            diag("--apply-mobileconfig is only supported on macOS");
            return ExitCode::from(1);
        }
    } else if opts.apply {
        match apply_mdm(target_os, &binary, &gateway_for_mdm) {
            Ok(summary) => {
                println!();
                println!("--- policy applied ({}) ---", os_label(target_os));
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

    #[cfg(target_os = "macos")]
    {
        match remove_macos_profile() {
            Ok(true) => println!("Removed managed profile {MACOS_PAYLOAD_IDENTIFIER}"),
            Ok(false) => println!(
                "No managed profile {MACOS_PAYLOAD_IDENTIFIER} installed (nothing to remove)"
            ),
            Err(e) => diag(&format!("profile remove failed: {e}")),
        }
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

#[cfg(target_os = "macos")]
fn remove_macos_profile() -> Result<bool, String> {
    use std::process::Command;
    let user = std::env::var("USER").unwrap_or_default();
    let user_path = format!("/Library/Managed Preferences/{user}/com.anthropic.claudefordesktop.plist");
    let sys_exists = Path::new(MACOS_MANAGED_PREFS_PATH).exists();
    let user_exists = !user.is_empty() && Path::new(&user_path).exists();

    // Try `profiles remove` first (cleans mobileconfig-installed payloads) — ok if it fails.
    let _ = Command::new("sudo")
        .args(["profiles", "remove", "-identifier", MACOS_PAYLOAD_IDENTIFIER])
        .status();

    if !sys_exists && !user_exists {
        return Ok(false);
    }

    let script = if user_exists {
        format!(
            r#"rm -f "{MACOS_MANAGED_PREFS_PATH}" "{user_path}"
/usr/bin/killall cfprefsd 2>/dev/null || true
"#
        )
    } else {
        format!(
            r#"rm -f "{MACOS_MANAGED_PREFS_PATH}"
/usr/bin/killall cfprefsd 2>/dev/null || true
"#
        )
    };
    let status = Command::new("sudo")
        .args(["sh", "-c", &script])
        .status()
        .map_err(|e| format!("sudo sh: {e}"))?;
    if !status.success() {
        return Err(format!(
            "sudo direct-remove exited with {}",
            status.code().unwrap_or(-1)
        ));
    }
    Ok(true)
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
pub(crate) const MACOS_PAYLOAD_IDENTIFIER: &str = "io.systemprompt.cowork.mdm";
#[cfg(target_os = "macos")]
const MACOS_INNER_PAYLOAD_IDENTIFIER: &str = "io.systemprompt.cowork.mdm.inference";
#[cfg(target_os = "macos")]
const MACOS_MANAGED_PREFS_PATH: &str =
    "/Library/Managed Preferences/com.anthropic.claudefordesktop.plist";

#[cfg(target_os = "macos")]
fn apply_macos(binary: &Path, gateway: &str) -> Result<Vec<String>, String> {
    use std::process::Command;

    validate_macos_gateway(gateway)?;

    // Direct-write path: bypasses `profiles install` (deprecated on macOS 11+ for
    // CLI-initiated installs; Apple now requires MDM channel or System Settings UI).
    // We write the raw prefs plist directly to the Managed Preferences location that
    // cfprefsd reads — works for a single-machine dev install with no MDM.
    let plist = build_macos_prefs_plist(binary, gateway);
    let tmp_path = std::env::temp_dir().join("systemprompt-cowork.prefs.plist");
    fs::write(&tmp_path, plist.as_bytes())
        .map_err(|e| format!("write {}: {e}", tmp_path.display()))?;

    let user = std::env::var("USER").unwrap_or_default();
    let tmp_str = tmp_path.to_string_lossy();
    let dest_system = MACOS_MANAGED_PREFS_PATH;
    let dest_user = format!("/Library/Managed Preferences/{user}/com.anthropic.claudefordesktop.plist");
    let script = if user.is_empty() {
        format!(
            r#"set -e
mkdir -p "/Library/Managed Preferences"
/usr/bin/install -m 0644 "{tmp_str}" "{dest_system}"
/usr/bin/killall cfprefsd 2>/dev/null || true
"#
        )
    } else {
        format!(
            r#"set -e
mkdir -p "/Library/Managed Preferences" "/Library/Managed Preferences/{user}"
/usr/bin/install -m 0644 "{tmp_str}" "{dest_system}"
/usr/bin/install -m 0644 "{tmp_str}" "{dest_user}"
/usr/bin/killall cfprefsd 2>/dev/null || true
"#
        )
    };

    let status = Command::new("sudo")
        .args(["sh", "-c", &script])
        .status()
        .map_err(|e| format!("sudo sh: {e}"))?;
    let _ = fs::remove_file(&tmp_path);
    if !status.success() {
        return Err(format!(
            "sudo direct-write exited with {}. \
             Re-run `systemprompt-cowork install --apply` and approve the sudo prompt, \
             or try `--apply-mobileconfig` for the MDM/System-Settings path.",
            status.code().unwrap_or(-1)
        ));
    }

    let mut summary = Vec::with_capacity(6);
    summary.push(format!("wrote: {dest_system}"));
    if !user.is_empty() {
        summary.push(format!("wrote: {dest_user}"));
    }
    summary.push(format!("gateway:           {gateway}"));
    summary.push(format!("credential helper: {}", binary.display()));
    summary.push("restarted cfprefsd (managed prefs picked up on next app launch)".into());
    summary.push(
        "Verify: defaults read /Library/Managed\\ Preferences/com.anthropic.claudefordesktop"
            .into(),
    );
    summary.push("Fully quit Cowork (Cmd+Q) and relaunch to pick up the new policy.".into());
    summary.push(String::new());
    summary.push("Next step — configure an upstream model at the gateway:".into());
    summary.push(
        "  Pointing Cowork at the gateway is half the flow. The gateway must also".into(),
    );
    summary.push(
        "  have a provider+model route that accepts the model id Cowork requests".into(),
    );
    summary.push(
        "  (e.g. claude-sonnet-4-6). If the gateway rejects the model, Cowork shows:".into(),
    );
    summary.push(
        "    \"There's an issue with the selected model (<id>). It may not exist...\"".into(),
    );
    summary.push(
        "  Configure upstream providers + model mappings at services/ai/config.yaml".into(),
    );
    summary.push(
        "  (API keys via env vars: ANTHROPIC_API_KEY / OPENAI_API_KEY / GEMINI_API_KEY)".into(),
    );
    summary.push("  and restart the gateway.".into());
    Ok(summary)
}

#[cfg(target_os = "macos")]
fn apply_macos_mobileconfig(binary: &Path, gateway: &str) -> Result<Vec<String>, String> {
    use std::process::Command;

    validate_macos_gateway(gateway)?;

    let mobileconfig = build_macos_mobileconfig(binary, gateway);
    let out_path = std::env::temp_dir().join("systemprompt-cowork.mobileconfig");
    fs::write(&out_path, mobileconfig.as_bytes())
        .map_err(|e| format!("write {}: {e}", out_path.display()))?;

    let opened = Command::new("open").arg(&out_path).status();

    let mut summary = Vec::with_capacity(5);
    summary.push(format!("wrote mobileconfig: {}", out_path.display()));
    summary.push(format!("payload identifier: {MACOS_PAYLOAD_IDENTIFIER}"));
    match opened {
        Ok(s) if s.success() => summary.push(
            "opened System Settings → Profiles — approve the profile there, then relaunch Cowork."
                .into(),
        ),
        _ => summary.push(format!(
            "could not auto-open System Settings; double-click {} manually.",
            out_path.display()
        )),
    }
    summary.push(
        "For fleet deployment, distribute this file via Jamf/Intune/Mosyle instead.".into(),
    );
    Ok(summary)
}

#[cfg(target_os = "macos")]
fn validate_macos_gateway(gateway: &str) -> Result<(), String> {
    if gateway.starts_with("http://")
        && !gateway.contains("://127.0.0.1")
        && !gateway.contains("://localhost")
    {
        return Err(format!(
            "gateway url {gateway} uses http:// for a non-loopback host; \
             Cowork rejects this. Use https:// or http://127.0.0.1:<port>."
        ));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn build_macos_prefs_plist(binary: &Path, gateway: &str) -> String {
    let binary_esc = xml_escape(&binary.to_string_lossy());
    let gateway_esc = xml_escape(gateway);
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>inferenceProvider</key><string>gateway</string>
  <key>inferenceGatewayBaseUrl</key><string>{gateway_esc}</string>
  <key>inferenceCredentialHelper</key><string>{binary_esc}</string>
  <key>inferenceCredentialHelperTtlSec</key><integer>3600</integer>
  <key>inferenceGatewayAuthScheme</key><string>bearer</string>
</dict>
</plist>
"#
    )
}

#[cfg(target_os = "macos")]
fn build_macos_mobileconfig(binary: &Path, gateway: &str) -> String {
    let binary_esc = xml_escape(&binary.to_string_lossy());
    let gateway_esc = xml_escape(gateway);
    let inner_uuid = stable_uuid(MACOS_INNER_PAYLOAD_IDENTIFIER);
    let outer_uuid = stable_uuid(MACOS_PAYLOAD_IDENTIFIER);

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>PayloadContent</key>
  <array>
    <dict>
      <key>PayloadType</key><string>com.anthropic.claudefordesktop</string>
      <key>PayloadIdentifier</key><string>{MACOS_INNER_PAYLOAD_IDENTIFIER}</string>
      <key>PayloadUUID</key><string>{inner_uuid}</string>
      <key>PayloadDisplayName</key><string>Claude Cowork Inference Gateway</string>
      <key>PayloadEnabled</key><true/>
      <key>PayloadVersion</key><integer>1</integer>
      <key>inferenceProvider</key><string>gateway</string>
      <key>inferenceGatewayBaseUrl</key><string>{gateway_esc}</string>
      <key>inferenceCredentialHelper</key><string>{binary_esc}</string>
      <key>inferenceCredentialHelperTtlSec</key><integer>3600</integer>
      <key>inferenceGatewayAuthScheme</key><string>bearer</string>
    </dict>
  </array>
  <key>PayloadType</key><string>Configuration</string>
  <key>PayloadIdentifier</key><string>{MACOS_PAYLOAD_IDENTIFIER}</string>
  <key>PayloadUUID</key><string>{outer_uuid}</string>
  <key>PayloadDisplayName</key><string>systemprompt-cowork inference routing</string>
  <key>PayloadDescription</key><string>Routes Claude Cowork inference through the configured gateway and credential helper.</string>
  <key>PayloadOrganization</key><string>systemprompt.io</string>
  <key>PayloadScope</key><string>System</string>
  <key>PayloadVersion</key><integer>1</integer>
  <key>PayloadRemovalDisallowed</key><false/>
</dict>
</plist>
"#
    )
}

#[cfg(target_os = "macos")]
fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(target_os = "macos")]
fn stable_uuid(seed: &str) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(seed.as_bytes());
    let b = &digest[..16];
    format!(
        "{:02X}{:02X}{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]
    )
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
