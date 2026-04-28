use serde::Deserialize;
use std::path::PathBuf;
use std::{env, fs};

const DEFAULT_GATEWAY_URL: &str = "http://localhost:8080";

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Config {
    pub gateway_url: Option<String>,
    #[serde(default)]
    pub pat: Option<PatConfig>,
    #[serde(default)]
    pub session: Option<SessionConfig>,
    #[serde(default)]
    pub mtls: Option<MtlsConfig>,
    #[serde(default)]
    pub sync: Option<SyncConfig>,
    #[serde(default)]
    pub claude: Option<ClaudeConfig>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ClaudeConfig {
    #[serde(default)]
    pub inference_gateway_base_url: Option<String>,
    #[serde(default)]
    pub auth_scheme: Option<String>,
    #[serde(default)]
    pub models: Option<Vec<String>>,
    #[serde(default)]
    pub organization_uuid: Option<String>,
}

const DEFAULT_INFERENCE_GATEWAY_BASE_URL: &str = "http://127.0.0.1:8080/v1";

pub fn claude_inference_base_url(cfg: &Config) -> String {
    cfg.claude
        .as_ref()
        .and_then(|c| c.inference_gateway_base_url.clone())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_INFERENCE_GATEWAY_BASE_URL.to_string())
}

pub fn claude_auth_scheme(cfg: &Config) -> String {
    cfg.claude
        .as_ref()
        .and_then(|c| c.auth_scheme.clone())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "bearer".to_string())
}

pub fn claude_models(cfg: &Config) -> Vec<String> {
    cfg.claude
        .as_ref()
        .and_then(|c| c.models.clone())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| {
            vec![
                "claude-opus-4-7".to_string(),
                "claude-sonnet-4-6".to_string(),
                "claude-haiku-4-5".to_string(),
            ]
        })
}

pub fn claude_organization_uuid(cfg: &Config) -> Option<String> {
    cfg.claude
        .as_ref()
        .and_then(|c| c.organization_uuid.clone())
        .filter(|s| !s.is_empty())
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PatConfig {
    #[serde(default)]
    pub file: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SessionConfig {
    #[serde(default)]
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct MtlsConfig {
    #[serde(default)]
    pub cert_keystore_ref: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SyncConfig {
    #[serde(default)]
    pub pinned_pubkey: Option<String>,
}

pub fn load() -> Config {
    let path = config_path();
    let mut cfg: Config = path
        .as_ref()
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default();

    if let Ok(url) = env::var("SP_COWORK_GATEWAY_URL") {
        cfg.gateway_url = Some(url);
    }
    if cfg.gateway_url.as_deref() == Some("") {
        cfg.gateway_url = None;
    }
    if cfg.gateway_url.is_none() {
        cfg.gateway_url = Some(DEFAULT_GATEWAY_URL.to_string());
    }

    apply_policy_pubkey(&mut cfg);
    cfg
}

fn apply_policy_pubkey(cfg: &mut Config) {
    let Some(policy_value) = policy_pubkey() else {
        return;
    };
    let sync = cfg.sync.get_or_insert_with(SyncConfig::default);
    match sync.pinned_pubkey.as_deref() {
        None => sync.pinned_pubkey = Some(policy_value),
        Some(existing) if existing == policy_value => {},
        Some(existing) => {
            tracing::warn!(
                operator_pubkey_prefix = %existing.chars().take(12).collect::<String>(),
                policy_pubkey_prefix = %policy_value.chars().take(12).collect::<String>(),
                "policy-provided manifest pubkey overrides operator-set value"
            );
            sync.pinned_pubkey = Some(policy_value);
        },
    }
}

pub fn gateway_url_or_default(cfg: &Config) -> String {
    cfg.gateway_url
        .clone()
        .unwrap_or_else(|| DEFAULT_GATEWAY_URL.to_string())
}

pub fn config_path() -> Option<PathBuf> {
    if let Ok(explicit) = env::var("SP_COWORK_CONFIG") {
        return Some(PathBuf::from(explicit));
    }
    let base = dirs::config_dir()?;
    Some(base.join("systemprompt").join("systemprompt-cowork.toml"))
}

pub fn ensure_gateway_url(url: &str) -> std::io::Result<()> {
    let path = config_path().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "config path unresolvable")
    })?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let existing = fs::read_to_string(&path).unwrap_or_default();
    if existing.contains("gateway_url") {
        return Ok(());
    }
    let mut next = existing;
    if !next.is_empty() && !next.ends_with('\n') {
        next.push('\n');
    }
    next.push_str(&format!("gateway_url = \"{url}\"\n"));
    fs::write(&path, next)
}

pub fn pinned_pubkey() -> Option<String> {
    load().sync.and_then(|s| s.pinned_pubkey)
}

pub fn policy_pubkey() -> Option<String> {
    if let Ok(value) = env::var("SP_COWORK_POLICY_PUBKEY") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    read_policy_pubkey_native()
}

#[cfg(target_os = "windows")]
fn read_policy_pubkey_native() -> Option<String> {
    use std::process::Command;
    let output = Command::new("reg")
        .args([
            "query",
            r"HKCU\SOFTWARE\Policies\Claude",
            "/v",
            "inferenceManifestPubkey",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("inferenceManifestPubkey") {
            let rest = rest.trim_start();
            let rest = rest.strip_prefix("REG_SZ").unwrap_or(rest).trim();
            if !rest.is_empty() {
                return Some(rest.to_string());
            }
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn read_policy_pubkey_native() -> Option<String> {
    use std::process::Command;
    let output = Command::new("defaults")
        .args([
            "read",
            "/Library/Managed Preferences/com.anthropic.claudefordesktop",
            "inferenceManifestPubkey",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if value.is_empty() { None } else { Some(value) }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn read_policy_pubkey_native() -> Option<String> {
    None
}

pub fn persist_pinned_pubkey(pubkey: &str) -> std::io::Result<()> {
    let path = config_path().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "config path unresolvable")
    })?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let existing = fs::read_to_string(&path).unwrap_or_default();
    let (before, _after) = strip_section(&existing, "[sync]");
    let mut next = before.trim_end().to_string();
    if !next.is_empty() {
        next.push_str("\n\n");
    }
    next.push_str(&format!("[sync]\npinned_pubkey = \"{pubkey}\"\n"));
    fs::write(&path, next)
}

fn strip_section<'a>(input: &'a str, header: &str) -> (&'a str, &'a str) {
    if let Some(start) = input.find(header) {
        let rest = &input[start..];
        let next_hdr = rest[header.len()..]
            .find("\n[")
            .map(|i| start + header.len() + i + 1);
        return (&input[..start], next_hdr.map_or("", |i| &input[i..]));
    }
    (input, "")
}
