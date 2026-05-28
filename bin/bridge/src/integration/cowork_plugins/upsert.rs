//! Atomic upserts of the four registry/settings files Cowork reads.
//!
//! Wraps the pure data layer (the `registry` and `settings` submodules) with
//! filesystem read/parse/render/write cycles. Foreign keys and foreign array
//! entries (the user's own marketplaces and plugin choices) MUST be preserved
//! by every helper here — see the per-function comments.

use std::path::Path;

use serde_json::Value;

use crate::fsutil;

use super::emit::{CoworkTarget, EmitError};
use super::{
    COWORK_SETTINGS_FILE, CoworkPluginsError, INSTALLED_PLUGINS_FILE, InstalledPluginEntry,
    KNOWN_MARKETPLACES_FILE, KnownMarketplaceEntry, LocalSource, MergeReport, enable_plugin,
    parse_root, parse_settings, render_settings, upsert_installed_plugin, upsert_known_marketplace,
};

pub(super) fn upsert_known(
    target: &CoworkTarget,
    mp_name: &str,
    now: &str,
) -> Result<MergeReport, EmitError> {
    let path = target.cowork_plugins_dir.join(KNOWN_MARKETPLACES_FILE);
    let mut root = parse_root_at(&path)?;
    let mp_path = target.marketplace_dir(mp_name);
    let mp_path_str = mp_path.to_string_lossy().into_owned();
    let entry = KnownMarketplaceEntry {
        name: mp_name.to_string(),
        source: LocalSource::local(mp_path_str.clone()),
        install_location: mp_path_str,
        last_updated: now.to_string(),
    };
    let report = upsert_known_marketplace(&mut root, &entry)?;
    write_root(&path, &root)?;
    Ok(report)
}

pub(super) fn upsert_installed(
    target: &CoworkTarget,
    mp_name: &str,
    plugin_name: &str,
    version: &str,
    install_path: &str,
    now: &str,
) -> Result<MergeReport, EmitError> {
    let path = target.cowork_plugins_dir.join(INSTALLED_PLUGINS_FILE);
    let mut root = parse_root_at(&path)?;
    let entry = InstalledPluginEntry {
        marketplace: mp_name.to_string(),
        name: plugin_name.to_string(),
        scope: "user".into(),
        install_path: install_path.to_string(),
        version: version.to_string(),
        installed_at: now.to_string(),
        last_updated: now.to_string(),
    };
    let report = upsert_installed_plugin(&mut root, &entry)?;
    write_root(&path, &root)?;
    Ok(report)
}

pub(super) fn upsert_enabled(
    target: &CoworkTarget,
    plugin_name: &str,
    mp_name: &str,
) -> Result<bool, EmitError> {
    let path = target.session_org_dir.join(COWORK_SETTINGS_FILE);
    let bytes = read_bytes(&path)?;
    let mut root = parse_settings(&bytes)?;
    let report = enable_plugin(&mut root, plugin_name, mp_name)?;
    atomic_write(&path, &render_settings(&root)?)?;
    Ok(report.set || report.already)
}

pub(super) fn retain_marketplaces(root: &mut serde_json::Map<String, Value>, drop_name: &str) {
    super::retain_known_marketplaces(root, drop_name);
}

pub(super) fn retain_installed(
    root: &mut serde_json::Map<String, Value>,
    drop_marketplace: &str,
    drop_name: &str,
) {
    let key = format!("{drop_name}@{drop_marketplace}");
    super::retain_installed_plugin(root, &key);
}

pub(super) fn inject_hooks_field(plugin_dir: &Path) -> Result<(), EmitError> {
    let hooks_dir = plugin_dir.join("hooks");
    if !hooks_dir.exists() {
        return Ok(());
    }
    let plugin_json = plugin_dir.join(".claude-plugin").join("plugin.json");
    let Some(text) = fsutil::read_optional(&plugin_json).map_err(|e| EmitError::Io {
        context: format!("read {}", plugin_json.display()),
        source: e,
    })?
    else {
        return Ok(());
    };
    let mut value: Value = serde_json::from_str(&text)
        .map_err(|e| EmitError::Data(CoworkPluginsError::JsonParse(e)))?;
    let Some(obj) = value.as_object_mut() else {
        return Ok(());
    };
    obj.insert("hooks".into(), Value::String("./hooks/hooks.json".into()));
    let out = serde_json::to_vec_pretty(&value)
        .map_err(|e| EmitError::Data(CoworkPluginsError::JsonParse(e)))?;
    atomic_write(&plugin_json, &out)
}

pub(super) fn current_iso8601() -> String {
    chrono::Utc::now().to_rfc3339()
}

pub(super) fn read_bytes(path: &Path) -> Result<Vec<u8>, EmitError> {
    Ok(fsutil::read_optional(path)
        .map_err(|e| EmitError::Io {
            context: format!("read {}", path.display()),
            source: e,
        })?
        .map(String::into_bytes)
        .unwrap_or_default())
}

pub(super) fn read_optional_bytes(path: &Path) -> Result<Option<Vec<u8>>, EmitError> {
    fsutil::read_optional(path)
        .map(|opt| opt.map(String::into_bytes))
        .map_err(|e| EmitError::Io {
            context: format!("read {}", path.display()),
            source: e,
        })
}

pub(super) fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), EmitError> {
    fsutil::atomic_write_0600(path, bytes).map_err(|e| EmitError::Io {
        context: format!("atomic_write {}", path.display()),
        source: e,
    })
}

fn parse_root_at(path: &Path) -> Result<serde_json::Map<String, Value>, EmitError> {
    let bytes = read_bytes(path)?;
    Ok(parse_root(&bytes)?)
}

fn write_root(path: &Path, root: &serde_json::Map<String, Value>) -> Result<(), EmitError> {
    let bytes = serde_json::to_vec_pretty(&Value::Object(root.clone()))?;
    atomic_write(path, &bytes)
}
