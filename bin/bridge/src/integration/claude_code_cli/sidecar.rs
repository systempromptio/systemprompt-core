//! The record of which Claude Code marketplaces this emitter owns.
//!
//! `~/.claude/plugins` is shared with marketplaces the user registers by hand,
//! and nothing in Claude Code's own registry files says who wrote an entry.
//! The sidecar is that record: a later sync prunes only the marketplaces it
//! lists, so a manifest that stops naming a marketplace removes exactly that
//! one and a user's own marketplaces survive every sync.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{LEGACY_MARKETPLACE, io_err};
use crate::host_sync::ApplyError;

pub const SIDECAR: &str = ".systemprompt-marketplaces.json";

#[derive(Debug, Default, Serialize, Deserialize)]
struct OwnedMarketplaces {
    marketplaces: Vec<String>,
}

fn read(plugins: &Path) -> Option<Vec<String>> {
    let bytes = std::fs::read(plugins.join(SIDECAR)).ok()?;
    serde_json::from_slice::<OwnedMarketplaces>(&bytes)
        .ok()
        .map(|s| s.marketplaces)
}

// Why: the sidecar's list plus the legacy single marketplace, which every
// bridge before the sidecar existed wrote without recording. Listing it here
// is what makes the first sync against a marketplace-aware gateway a one-way
// purge of the old layout.
#[must_use]
pub fn previously_owned(plugins: &Path) -> Vec<String> {
    let mut owned = read(plugins).unwrap_or_default();
    if !owned.iter().any(|id| id == LEGACY_MARKETPLACE) {
        owned.push(LEGACY_MARKETPLACE.to_owned());
    }
    owned
}

// Why: a diagnostic inspects the sidecar's list, or the legacy marketplace
// when no sidecar has been written yet — the only layout that could exist.
#[must_use]
pub fn owned_marketplaces(plugins: &Path) -> Vec<String> {
    match read(plugins) {
        Some(owned) if !owned.is_empty() => owned,
        _ => vec![LEGACY_MARKETPLACE.to_owned()],
    }
}

pub fn write(plugins: &Path, marketplaces: &[String]) -> Result<(), ApplyError> {
    let state = OwnedMarketplaces {
        marketplaces: marketplaces.to_vec(),
    };
    super::json_io::write_json(
        &plugins.join(SIDECAR),
        &serde_json::to_value(state).map_err(|e| ApplyError::Serialize {
            what: "claude-code marketplaces sidecar".into(),
            source: e,
        })?,
    )
}

pub fn remove(plugins: &Path) -> Result<(), ApplyError> {
    let path = plugins.join(SIDECAR);
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(io_err(format!("remove {}", path.display()), e)),
    }
}
