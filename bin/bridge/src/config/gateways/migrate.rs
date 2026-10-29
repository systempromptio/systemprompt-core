//! One-way move from the single-gateway layout into `gateways/default/`.
//!
//! The legacy layout kept the whole config at the index path and the PAT in a
//! file beside it. The move writes the new profile first — PAT into its store
//! and read back, then the config — and only then replaces the legacy file
//! with the index. That replacement is the commit point: a crash before it
//! re-runs the copy, a crash after it leaves only the legacy PAT file, which
//! the next run removes once the stored copy reads back equal.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use toml_edit::DocumentMut;

use super::credential::{self, PatStore};
use super::{GatewayProfileId, ProfileError, index, profile_for};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MigrationOutcome {
    Pinned,
    Fresh,
    AlreadyMigrated,
    Migrated {
        profile: GatewayProfileId,
        pat: Option<PatStore>,
    },
}

pub fn migrate_legacy_layout() -> Result<MigrationOutcome, ProfileError> {
    run(credential::select_store)
}

pub fn migrate_legacy_layout_with(store: PatStore) -> Result<MigrationOutcome, ProfileError> {
    run(|| store)
}

fn run(choose: impl FnOnce() -> PatStore) -> Result<MigrationOutcome, ProfileError> {
    if super::pinned_config().is_some() {
        return Ok(MigrationOutcome::Pinned);
    }
    let root = super::config_root().ok_or(ProfileError::PathUnresolvable)?;
    let index_path = super::index_path().ok_or(ProfileError::PathUnresolvable)?;
    let legacy_pat = root.join(crate::brand::brand().pat_file);
    let Some(doc) = index::read_doc(&index_path)? else {
        return Ok(MigrationOutcome::Fresh);
    };
    if let Some(active) = index::active_of(&index_path, &doc)? {
        remove_leftover_pat(&legacy_pat, &active)?;
        return Ok(MigrationOutcome::AlreadyMigrated);
    }
    migrate(&index_path, doc, &legacy_pat, choose)
}

fn migrate(
    index_path: &Path,
    mut doc: DocumentMut,
    default_pat: &Path,
    choose: impl FnOnce() -> PatStore,
) -> Result<MigrationOutcome, ProfileError> {
    let id = GatewayProfileId::default_profile();
    let profile = profile_for(id.clone())?;
    crate::fsutil::create_dir_all_mode_0700(&profile.dir)
        .map_err(|e| ProfileError::io("create", &profile.dir, e))?;

    let pat_path = legacy_pat_path(&doc).unwrap_or_else(|| default_pat.to_path_buf());
    let token = crate::fsutil::read_optional(&pat_path)
        .map_err(|e| ProfileError::io("read", &pat_path, e))?
        .map(|t| t.trim().to_owned())
        .filter(|t| !t.is_empty());

    crate::config::write::remove(&mut doc, &["pat"])?;
    let store = match token.as_deref() {
        Some(token) => {
            let store = choose();
            credential::write(&profile, store, token)?;
            crate::config::write::set(&mut doc, &["pat", "store"], store.as_str())?;
            Some(store)
        },
        None => None,
    };

    let body = doc.to_string();
    crate::fsutil::atomic_write_0600(&profile.config_file, body.as_bytes())
        .map_err(|e| ProfileError::io("write", &profile.config_file, e))?;
    crate::config::read_file(&profile.config_file)?;

    index::write_active(index_path, &id)?;

    if token.is_some() {
        crate::fsutil::remove_verified(&pat_path)
            .map_err(|e| ProfileError::io("remove", &pat_path, e))?;
    }
    tracing::info!(
        profile = %id,
        pat_store = store.map_or("none", PatStore::as_str),
        "moved the single-gateway config into a gateway profile"
    );
    Ok(MigrationOutcome::Migrated {
        profile: id,
        pat: store,
    })
}

fn legacy_pat_path(doc: &DocumentMut) -> Option<PathBuf> {
    crate::config::write::get(doc, &["pat", "file"])
        .and_then(|item| item.as_str())
        .map(|p| PathBuf::from(crate::fsutil::expand_tilde(p)))
}

fn remove_leftover_pat(legacy_pat: &Path, active: &GatewayProfileId) -> Result<(), ProfileError> {
    let Some(leftover) = crate::fsutil::read_optional(legacy_pat)
        .map_err(|e| ProfileError::io("read", legacy_pat, e))?
    else {
        return Ok(());
    };
    let profile = profile_for(GatewayProfileId::default_profile())?;
    let cfg = crate::config::read_file(&profile.config_file)?;
    let stored = match cfg.pat.as_ref().and_then(|p| p.store) {
        Some(store) => credential::read(&profile, store)?,
        None => None,
    };
    if stored
        .as_ref()
        .is_some_and(|s| s.as_str() == leftover.trim())
    {
        crate::fsutil::remove_verified(legacy_pat)
            .map_err(|e| ProfileError::io("remove", legacy_pat, e))?;
        tracing::info!(profile = %active, "removed the legacy PAT file left by an interrupted move");
    } else {
        tracing::warn!(
            path = %legacy_pat.display(),
            "a legacy PAT file remains that does not match the migrated credential; it is not read"
        );
    }
    Ok(())
}
