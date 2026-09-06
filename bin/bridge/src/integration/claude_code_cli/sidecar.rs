//! The record of which Claude Code marketplaces this emitter owns.
//!
//! `~/.claude/plugins` is shared with marketplaces the user registers by hand,
//! and nothing in Claude Code's own registry files says who wrote an entry.
//! The sidecar is that record: a later sync prunes only the marketplaces it
//! lists, so a manifest that stops naming a marketplace removes exactly that
//! one and a user's own marketplaces survive every sync.
//!
//! An absent sidecar is a normal state and reads as "nothing recorded"; a
//! present but unparseable one is an error, because treating it as absent
//! would silently narrow the purge to the legacy marketplace and orphan every
//! other marketplace this emitter wrote.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::MarketplaceId;

use super::{io_err, legacy_marketplace_id};
use crate::fsutil;
use crate::host_sync::ApplyError;

pub const SIDECAR: &str = ".systemprompt-marketplaces.json";

/// How the legacy marketplace is folded into a sidecar read.
///
/// `Always` makes a first sync against a marketplace-aware gateway purge the
/// single-marketplace layout every bridge wrote before the sidecar existed.
/// `WhenUnrecorded` names it only when nothing is recorded, the sole layout
/// that could exist before the first sidecar write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Legacy {
    Always,
    WhenUnrecorded,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct OwnedMarketplaces {
    marketplaces: Vec<MarketplaceId>,
}

fn read(plugins: &Path) -> Result<Vec<MarketplaceId>, ApplyError> {
    let path = plugins.join(SIDECAR);
    let Some(text) =
        fsutil::read_optional(&path).map_err(|e| io_err(format!("read {}", path.display()), e))?
    else {
        return Ok(Vec::new());
    };
    serde_json::from_str::<OwnedMarketplaces>(&text)
        .map(|s| s.marketplaces)
        .map_err(|e| {
            io_err(
                format!(
                    "parse {}; refusing to treat a corrupt sidecar as absent",
                    path.display()
                ),
                std::io::Error::other(e),
            )
        })
}

pub fn owned_marketplaces(
    plugins: &Path,
    legacy: Legacy,
) -> Result<Vec<MarketplaceId>, ApplyError> {
    let mut owned = read(plugins)?;
    let legacy_id = legacy_marketplace_id();
    match legacy {
        Legacy::Always if !owned.contains(&legacy_id) => owned.push(legacy_id),
        Legacy::WhenUnrecorded if owned.is_empty() => owned.push(legacy_id),
        _ => {},
    }
    Ok(owned)
}

pub fn write(plugins: &Path, marketplaces: &[MarketplaceId]) -> Result<(), ApplyError> {
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
