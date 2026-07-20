//! Atomic upserts of `cowork_settings.json::enabledPlugins`, preserving every
//! foreign key.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use crate::fsutil;

use super::emit::{CoworkTarget, EmitError};
use super::{
    COWORK_SETTINGS_FILE, disable_plugin, parse_settings, reconcile_marketplace, render_settings,
};

pub(super) fn reconcile_enabled(
    target: &CoworkTarget,
    plugins: &[&str],
    mp_name: &str,
) -> Result<bool, EmitError> {
    let path = target.session_org_dir.join(COWORK_SETTINGS_FILE);
    let bytes = read_bytes(&path)?;
    let mut root = parse_settings(&bytes)?;
    let changed = reconcile_marketplace(&mut root, plugins, mp_name)?;
    if changed {
        atomic_write(&path, &render_settings(&root)?)?;
    }
    Ok(changed)
}

pub(super) fn clear_enabled(
    target: &CoworkTarget,
    plugin_name: &str,
    mp_name: &str,
) -> Result<(), EmitError> {
    let path = target.session_org_dir.join(COWORK_SETTINGS_FILE);
    let Some(bytes) = read_optional_bytes(&path)? else {
        return Ok(());
    };
    let mut root = parse_settings(&bytes)?;
    if disable_plugin(&mut root, plugin_name, mp_name)? {
        atomic_write(&path, &render_settings(&root)?)?;
    }
    Ok(())
}

fn read_bytes(path: &Path) -> Result<Vec<u8>, EmitError> {
    Ok(fsutil::read_optional(path)
        .map_err(|e| EmitError::Io {
            context: format!("read {}", path.display()),
            source: e,
        })?
        .map(String::into_bytes)
        .unwrap_or_default())
}

fn read_optional_bytes(path: &Path) -> Result<Option<Vec<u8>>, EmitError> {
    fsutil::read_optional(path)
        .map(|opt| opt.map(String::into_bytes))
        .map_err(|e| EmitError::Io {
            context: format!("read {}", path.display()),
            source: e,
        })
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), EmitError> {
    fsutil::atomic_write_0600(path, bytes).map_err(|e| EmitError::Io {
        context: format!("atomic_write {}", path.display()),
        source: e,
    })
}
