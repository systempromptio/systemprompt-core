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
    cfg
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
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "config path unresolvable",
        )
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

pub fn persist_pinned_pubkey(pubkey: &str) -> std::io::Result<()> {
    let path = config_path().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "config path unresolvable",
        )
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
