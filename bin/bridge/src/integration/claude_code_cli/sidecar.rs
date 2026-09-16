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

use super::io_err;
use crate::fsutil;
use crate::host_sync::ApplyError;

pub const SIDECAR: &str = ".systemprompt-marketplaces.json";

/// Everything this emitter wrote that a later run must be able to take back.
///
/// The mirrored marketplaces, the dependency keys enabled on their behalf in
/// `settings.json`, and the foreign marketplaces registered for those
/// dependencies.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Owned {
    pub marketplaces: Vec<MarketplaceId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependency_keys: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub external_marketplaces: Vec<String>,
}

pub fn read(plugins: &Path) -> Result<Owned, ApplyError> {
    let path = plugins.join(SIDECAR);
    let Some(text) =
        fsutil::read_optional(&path).map_err(|e| io_err(format!("read {}", path.display()), e))?
    else {
        return Ok(Owned::default());
    };
    serde_json::from_str::<Owned>(&text).map_err(|e| {
        io_err(
            format!(
                "parse {}; refusing to treat a corrupt sidecar as absent",
                path.display()
            ),
            std::io::Error::other(e),
        )
    })
}

pub fn owned_marketplaces(plugins: &Path) -> Result<Vec<MarketplaceId>, ApplyError> {
    read(plugins).map(|owned| owned.marketplaces)
}

pub fn write(plugins: &Path, owned: &Owned) -> Result<(), ApplyError> {
    crate::integration::json_io::write_json(
        &plugins.join(SIDECAR),
        &serde_json::to_value(owned).map_err(|e| ApplyError::Serialize {
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
