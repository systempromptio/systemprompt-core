//! Which tier owns the `systemprompt` provider: an admin-managed tier (MDM)
//! or the user's own config.
//!
//! An admin who deploys `OpenCode` through MDM puts the provider and its
//! policies in the managed tier (`/etc/opencode`, `/Library/Application
//! Support/opencode`, `%ProgramData%\opencode`, or the macOS
//! `ai.opencode.managed` managed-preferences plist). The bridge then writes no
//! provider or policy config of its own — only the token file the managed
//! config names with `{file:~/.config/opencode/systemprompt.token}`, and
//! content.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use serde_json::Value;

use super::config;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ProviderTier {
    Managed(PathBuf),
    User,
}

pub(super) fn provider_tier() -> ProviderTier {
    let Ok(dir) = config::managed_dir() else {
        return ProviderTier::User;
    };
    for name in [config::CONFIG_FILE, config::CONFIG_FILE_JSONC] {
        let path = dir.join(name);
        if path != config::user_config_path() && carries_provider(&path) {
            return ProviderTier::Managed(path);
        }
    }
    let plist = PathBuf::from(config::MANAGED_PLIST);
    if cfg!(target_os = "macos") && plist_names_provider(&plist) {
        return ProviderTier::Managed(plist);
    }
    ProviderTier::User
}

// JSON: OpenCode's config file is host-owned JSON with an open schema; only
// the provider tables are inspected.
fn carries_provider(path: &Path) -> bool {
    let Ok(Some(text)) = crate::fsutil::read_optional(path) else {
        return false;
    };
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    let Ok(value) = serde_json::from_str::<Value>(strip_line_comments(text).as_str()) else {
        return false;
    };
    ["providers", "provider"].iter().any(|table| {
        value
            .get(table)
            .and_then(|providers| providers.get(config::PROVIDER_ID))
            .is_some()
    })
}

// Why: `opencode.jsonc` may carry `//` comments; a full-line comment is the
// only form stripped, which is enough to read a provider table.
fn strip_line_comments(text: &str) -> String {
    text.lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

// Why: a managed-preferences plist is XML or binary; either stores an ASCII
// key verbatim, so the provider id is found without a plist parser.
fn plist_names_provider(path: &Path) -> bool {
    std::fs::read(path).is_ok_and(|bytes| {
        bytes
            .windows(config::PROVIDER_ID.len())
            .any(|window| window == config::PROVIDER_ID.as_bytes())
    })
}
