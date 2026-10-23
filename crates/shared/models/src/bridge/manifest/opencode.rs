//! The `OpenCode` client settings a marketplace asks the bridge to apply.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

use crate::bridge::ids::Sha256Digest;

// Why: a bridge older than the V2 `OpenCode` writer would project a V1 config
// over a V2 client, so a marketplace carrying `opencode` settings requires it.
pub const OPENCODE_V2_MIN_BRIDGE: semver::Version = semver::Version::new(0, 73, 0);

/// Tolerant of unknown keys, unlike the kit-side `OpenCodeMarketplaceConfig`
/// (services manifest), so a bridge ignores a setting a newer gateway adds.
///
/// `cli_defaults` are top-level `opencode.json` keys the bridge sets only where
/// the user has not set them; `theme` names a theme file the plugin bundles
/// ship under `hosts/opencode/themes/<name>.json`, pinned by digest.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestOpenCode {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_client_version: Option<semver::Version>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<ThemeRef>,
    #[serde(default, skip_serializing_if = "serde_json::Map::is_empty")]
    // JSON: client defaults are foreign `opencode.json` keys, passed through
    pub cli_defaults: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemeRef {
    pub name: String,
    pub sha: Sha256Digest,
}
