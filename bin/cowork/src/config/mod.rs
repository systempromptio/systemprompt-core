pub mod paths;

use serde::Deserialize;
use std::path::PathBuf;
use std::sync::Once;
use std::{env, fs};

use systemprompt_identifiers::ValidatedUrl;

use crate::ids::{KeystoreRef, PinnedPubKey};

const DEFAULT_GATEWAY_URL: &str = "http://localhost:8080";

fn default_gateway_url() -> ValidatedUrl {
    #[allow(clippy::expect_used)]
    ValidatedUrl::try_new(DEFAULT_GATEWAY_URL).expect("DEFAULT_GATEWAY_URL is a valid URL")
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub gateway_url: Option<ValidatedUrl>,
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
    pub inference_gateway_base_url: Option<ValidatedUrl>,
    #[serde(default)]
    pub auth_scheme: Option<String>,
    #[serde(default)]
    pub models: Option<Vec<String>>,
    #[serde(default)]
    pub organization_uuid: Option<String>,
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
    pub cert_keystore_ref: Option<KeystoreRef>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SyncConfig {
    #[serde(default)]
    pub pinned_pubkey: Option<PinnedPubKey>,
}

impl Config {
    pub fn load() -> Self {
        let path = config_path();
        let mut cfg: Config = path
            .as_ref()
            .and_then(|p| fs::read_to_string(p).ok())
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default();

        if let Ok(url) = env::var("SP_COWORK_GATEWAY_URL") {
            if let Ok(parsed) = ValidatedUrl::try_new(url.trim()) {
                cfg.gateway_url = Some(parsed);
            }
        }
        if cfg.gateway_url.is_none() {
            cfg.gateway_url = Some(default_gateway_url());
        }

        cfg
    }

    pub fn with_policy_overrides(mut self) -> Self {
        let Some(policy_value) = policy_pubkey() else {
            return self;
        };
        let sync = self.sync.get_or_insert_with(SyncConfig::default);
        match sync.pinned_pubkey.as_ref() {
            None => sync.pinned_pubkey = Some(policy_value),
            Some(existing) if existing.as_str() == policy_value.as_str() => {},
            Some(existing) => {
                static WARN_ONCE: Once = Once::new();
                let existing_prefix: String = existing.as_str().chars().take(12).collect();
                let policy_prefix: String = policy_value.as_str().chars().take(12).collect();
                WARN_ONCE.call_once(|| {
                    tracing::warn!(
                        operator_pubkey_prefix = %existing_prefix,
                        policy_pubkey_prefix = %policy_prefix,
                        "policy-provided manifest pubkey overrides operator-set value"
                    );
                });
                sync.pinned_pubkey = Some(policy_value);
            },
        }
        self
    }

    #[must_use]
    pub fn cert_keystore_ref(&self) -> Option<&KeystoreRef> {
        self.mtls
            .as_ref()
            .and_then(|m| m.cert_keystore_ref.as_ref())
    }
}

pub fn load() -> Config {
    Config::load().with_policy_overrides()
}

#[must_use]
pub fn gateway_url_or_default(cfg: &Config) -> ValidatedUrl {
    cfg.gateway_url.clone().unwrap_or_else(default_gateway_url)
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

#[must_use]
pub fn pinned_pubkey() -> Option<PinnedPubKey> {
    load().sync.and_then(|s| s.pinned_pubkey)
}

#[must_use]
pub fn policy_pubkey() -> Option<PinnedPubKey> {
    if let Ok(value) = env::var("SP_COWORK_POLICY_PUBKEY") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Some(PinnedPubKey::new(trimmed));
        }
    }
    read_policy_pubkey_native().map(PinnedPubKey::new)
}

#[cfg(target_os = "windows")]
fn read_policy_pubkey_native() -> Option<String> {
    let output = crate::winproc::reg_command()
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

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_full_config_preserves_wire_format() {
        let toml_input = r#"gateway_url = "https://gateway.example.com"

[pat]
file = "/etc/cowork/pat.token"

[session]
enabled = true

[mtls]
cert_keystore_ref = "macos:my-cert-label"

[sync]
pinned_pubkey = "MCowBQYDK2VwAyEABase64Pubkey=="

[claude]
inference_gateway_base_url = "https://inference.example.com"
auth_scheme = "bearer"
models = ["claude-opus-4", "claude-sonnet-4"]
organization_uuid = "abc-123"
"#;
        let cfg: Config = toml::from_str(toml_input).expect("parse toml");
        assert_eq!(
            cfg.gateway_url.as_ref().map(ValidatedUrl::as_str),
            Some("https://gateway.example.com"),
        );
        assert_eq!(
            cfg.mtls
                .as_ref()
                .and_then(|m| m.cert_keystore_ref.as_ref())
                .map(KeystoreRef::as_str),
            Some("macos:my-cert-label"),
        );
        assert_eq!(
            cfg.sync
                .as_ref()
                .and_then(|s| s.pinned_pubkey.as_ref())
                .map(PinnedPubKey::as_str),
            Some("MCowBQYDK2VwAyEABase64Pubkey=="),
        );
        assert_eq!(
            cfg.claude
                .as_ref()
                .and_then(|c| c.inference_gateway_base_url.as_ref())
                .map(ValidatedUrl::as_str),
            Some("https://inference.example.com"),
        );
    }

    #[test]
    fn empty_inference_gateway_base_url_rejected() {
        let toml_input = r#"
[claude]
inference_gateway_base_url = ""
"#;
        let result: Result<Config, _> = toml::from_str(toml_input);
        assert!(result.is_err(), "empty ValidatedUrl must fail validation");
    }
}
