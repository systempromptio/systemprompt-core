//! IO layer for the Cowork desktop marketplace integration.
//!
//! Wraps the pure data layer (the `registry`, `settings`, and `marketplace`
//! sibling submodules) with filesystem operations: session/org resolution,
//! recursive plugin-tree copies into both the marketplace and cache
//! locations, and atomic upserts of the four registry/settings files.
//!
//! Atomic writes, recursive copies, and optional reads delegate to
//! [`crate::fsutil`]; per-file upsert plumbing lives in the sibling
//! `upsert` submodule.

use std::path::{Path, PathBuf};
use std::time::SystemTime;
use std::{fs, io};

use serde_json::Value;

use crate::config::paths;
use crate::fsutil;

use super::upsert::{
    atomic_write, current_iso8601, inject_hooks_field, read_optional_bytes, retain_installed,
    retain_marketplaces, upsert_enabled, upsert_installed, upsert_known,
};
use super::{
    COWORK_SETTINGS_FILE, CoworkPluginsError, INSTALLED_PLUGINS_FILE, KNOWN_MARKETPLACES_FILE,
    LocalSource, MarketplaceFile, MarketplaceOwner, MarketplacePluginEntry, parse_root,
    parse_settings, render_marketplace, render_settings,
};

#[derive(Debug, thiserror::Error)]
pub enum EmitError {
    #[error("io error in {context}: {source}")]
    Io {
        context: String,
        #[source]
        source: io::Error,
    },
    #[error("data error: {0}")]
    Data(#[from] CoworkPluginsError),
    #[error("json serialize: {0}")]
    Json(#[from] serde_json::Error),
    #[error("source plugin tree missing at {0}")]
    SourceMissing(PathBuf),
}

#[derive(Debug, Clone)]
pub struct CoworkTarget {
    pub session_org_dir: PathBuf,
    pub cowork_plugins_dir: PathBuf,
}

impl CoworkTarget {
    pub(super) fn marketplace_dir(&self, mp_name: &str) -> PathBuf {
        self.cowork_plugins_dir.join("marketplaces").join(mp_name)
    }

    fn cache_dir(&self, mp_name: &str, plugin_name: &str, version: &str) -> PathBuf {
        self.cowork_plugins_dir
            .join("cache")
            .join(mp_name)
            .join(plugin_name)
            .join(version)
    }
}

#[expect(
    clippy::struct_excessive_bools,
    reason = "EmitReport is a structured status report; each flag tracks a distinct sync sub-step"
)]
#[derive(Debug, Default, Clone)]
pub struct EmitReport {
    pub target: Option<PathBuf>,
    pub plugin_copied: bool,
    pub marketplace_registered: bool,
    pub plugin_installed_registered: bool,
    pub enabled: bool,
}

// `None` means no Cowork install detected — callers must treat as no-op,
// not as an error (Cowork is optional).
#[must_use]
pub fn resolve_target() -> Option<CoworkTarget> {
    let sessions_root = paths::cowork3p_sessions_root()?;
    if !sessions_root.is_dir() {
        return None;
    }
    let mut candidates: Vec<(SystemTime, PathBuf)> = Vec::new();
    for session in fs::read_dir(&sessions_root).ok()?.flatten() {
        if !session.file_type().is_ok_and(|t| t.is_dir()) {
            continue;
        }
        let Ok(orgs) = fs::read_dir(session.path()) else {
            continue;
        };
        for org in orgs.flatten() {
            if !org.file_type().is_ok_and(|t| t.is_dir()) {
                continue;
            }
            let path = org.path();
            let mtime = fs::metadata(&path)
                .and_then(|m| m.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            candidates.push((mtime, path));
        }
    }
    candidates.sort_by_key(|(t, _)| std::cmp::Reverse(*t));
    candidates.into_iter().next().map(|(_, session_org_dir)| {
        let cowork_plugins_dir = session_org_dir.join(paths::COWORK_PLUGINS_SUBDIR);
        CoworkTarget {
            session_org_dir,
            cowork_plugins_dir,
        }
    })
}

pub fn publish(
    target: &CoworkTarget,
    org_plugins_root: &Path,
    plugin_name: &str,
    version: &str,
    description: Option<&str>,
) -> Result<EmitReport, EmitError> {
    let source = org_plugins_root.join(plugin_name);
    if !source.is_dir() {
        return Err(EmitError::SourceMissing(source));
    }
    let mut report = EmitReport {
        target: Some(target.session_org_dir.clone()),
        ..EmitReport::default()
    };

    let mp_name = paths::BRIDGE_MARKETPLACE_NAME;

    let mp_plugin_dir = target
        .marketplace_dir(mp_name)
        .join("plugins")
        .join(plugin_name);
    let cache_plugin_dir = target.cache_dir(mp_name, plugin_name, version);

    copy_tree(&source, &mp_plugin_dir)?;
    copy_tree(&source, &cache_plugin_dir)?;
    inject_hooks_field(&mp_plugin_dir)?;
    inject_hooks_field(&cache_plugin_dir)?;
    report.plugin_copied = true;

    let mp_meta = target.marketplace_dir(mp_name).join(".claude-plugin");
    let mp_file = MarketplaceFile {
        name: mp_name.to_string(),
        owner: MarketplaceOwner {
            name: "systemprompt.io".into(),
            email: None,
        },
        plugins: vec![MarketplacePluginEntry {
            name: plugin_name.to_string(),
            source: LocalSource::local(format!("./plugins/{plugin_name}")),
            version: version.to_string(),
            description: description.map(str::to_string),
        }],
    };
    let bytes = render_marketplace(&mp_file)?;
    atomic_write(&mp_meta.join("marketplace.json"), &bytes)?;
    report.marketplace_registered = true;

    let now = current_iso8601();
    upsert_known(target, mp_name, &now)?;
    let installed_report = upsert_installed(target, mp_name, plugin_name, version, &now)?;
    report.plugin_installed_registered = !installed_report
        .unchanged
        .contains(&format!("{mp_name}::{plugin_name}"));

    let enabled = upsert_enabled(target, plugin_name, mp_name)?;
    report.enabled = enabled;

    tracing::info!(
        target: "bridge::cowork",
        session_org = %target.session_org_dir.display(),
        plugin = plugin_name,
        marketplace = mp_name,
        version = version,
        "published bridge plugin to Cowork marketplace"
    );

    Ok(report)
}

pub fn unpublish(target: &CoworkTarget, plugin_name: &str) -> Result<(), EmitError> {
    let mp_name = paths::BRIDGE_MARKETPLACE_NAME;

    let mp_dir = target.marketplace_dir(mp_name);
    if mp_dir.exists() {
        fs::remove_dir_all(&mp_dir).map_err(|e| EmitError::Io {
            context: format!("remove {}", mp_dir.display()),
            source: e,
        })?;
    }
    let cache_root = target.cowork_plugins_dir.join("cache").join(mp_name);
    if cache_root.exists() {
        fs::remove_dir_all(&cache_root).map_err(|e| EmitError::Io {
            context: format!("remove {}", cache_root.display()),
            source: e,
        })?;
    }

    let known_path = target.cowork_plugins_dir.join(KNOWN_MARKETPLACES_FILE);
    if let Some(bytes) = read_optional_bytes(&known_path)? {
        let mut root = parse_root(&bytes)?;
        retain_marketplaces(&mut root, mp_name);
        atomic_write(
            &known_path,
            &serde_json::to_vec_pretty(&Value::Object(root))?,
        )?;
    }

    let installed_path = target.cowork_plugins_dir.join(INSTALLED_PLUGINS_FILE);
    if let Some(bytes) = read_optional_bytes(&installed_path)? {
        let mut root = parse_root(&bytes)?;
        retain_installed(&mut root, mp_name, plugin_name);
        atomic_write(
            &installed_path,
            &serde_json::to_vec_pretty(&Value::Object(root))?,
        )?;
    }

    let settings_path = target.session_org_dir.join(COWORK_SETTINGS_FILE);
    if let Some(bytes) = read_optional_bytes(&settings_path)? {
        let mut root = parse_settings(&bytes)?;
        super::disable_plugin(&mut root, plugin_name, mp_name)?;
        atomic_write(&settings_path, &render_settings(&root)?)?;
    }

    Ok(())
}

fn copy_tree(src: &Path, dst: &Path) -> Result<(), EmitError> {
    if !src.is_dir() {
        return Err(EmitError::SourceMissing(src.to_path_buf()));
    }
    if dst.exists() {
        fs::remove_dir_all(dst).map_err(|e| EmitError::Io {
            context: format!("clear {}", dst.display()),
            source: e,
        })?;
    }
    fsutil::copy_dir_recursive(src, dst).map_err(|e| EmitError::Io {
        context: format!("copy {} → {}", src.display(), dst.display()),
        source: e,
    })
}
