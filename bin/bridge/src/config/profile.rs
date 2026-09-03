//! Bridge profile parsing, including the native policy public key section.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Deserialize;
use std::env;

use systemprompt_identifiers::ValidatedUrl;

use super::{Config, default_gateway};
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

#[must_use]
pub fn gateway_url_or_default(cfg: &Config) -> ValidatedUrl {
    let url = cfg.gateway_url.clone().unwrap_or_else(default_gateway);
    tracing::debug!(gateway = %url, "gateway resolved");
    url
}

#[must_use]
pub fn pinned_pubkey() -> Option<PinnedPubKey> {
    super::load().sync.and_then(|s| s.pinned_pubkey)
}

#[must_use]
pub fn policy_pubkey() -> Option<PinnedPubKey> {
    if let Ok(value) = env::var(crate::brand::brand().env("POLICY_PUBKEY")) {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Some(PinnedPubKey::new(trimmed));
        }
    }
    read_policy_pubkey_native().map(PinnedPubKey::new)
}

fn read_policy_pubkey_native() -> Option<String> {
    let value = super::store::read_bridge_policy(super::store::MANIFEST_PUBKEY_KEY)?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

pub fn persist_pinned_pubkey(pubkey: &str) -> Result<(), super::ConfigWriteError> {
    super::write::edit(|doc| super::write::set(doc, &["sync", "pinned_pubkey"], pubkey))
}
