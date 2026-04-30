use serde::Deserialize;
use std::fmt::Write as _;
use std::{env, fs};

use systemprompt_identifiers::ValidatedUrl;

use super::{Config, DEFAULT_GATEWAY, config_path};
use crate::ids::PinnedPubKey;

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

pub(super) fn default_gateway_url() -> ValidatedUrl {
    DEFAULT_GATEWAY.clone()
}

#[must_use]
pub fn gateway_url_or_default(cfg: &Config) -> ValidatedUrl {
    cfg.gateway_url.clone().unwrap_or_else(default_gateway_url)
}

#[must_use]
pub fn pinned_pubkey() -> Option<PinnedPubKey> {
    super::load().sync.and_then(|s| s.pinned_pubkey)
}

#[must_use]
pub fn policy_pubkey() -> Option<PinnedPubKey> {
    if let Ok(value) = env::var("SP_BRIDGE_POLICY_PUBKEY") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Some(PinnedPubKey::new(trimmed));
        }
    }
    read_policy_pubkey_native().map(PinnedPubKey::new)
}

fn read_policy_pubkey_native() -> Option<String> {
    let store = super::store::managed_policy_store();
    let value = store
        .read_managed_policy("inferenceManifestPubkey")
        .ok()
        .flatten()?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
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
    let _ = writeln!(next, "[sync]\npinned_pubkey = \"{pubkey}\"");
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
