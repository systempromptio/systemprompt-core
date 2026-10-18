//! Profile `paths:` block resolving system/services locations.
//!
//! `data` is the node-local root for state the server writes for itself (the
//! gateway accounting journal and scratch trees). It defaults to
//! `<storage>/data`; a profile whose `storage` is a mount shared between
//! replicas must point it at a per-node volume, because journal receipts
//! belong to the node that admitted them.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Filesystem roots for one profile.
///
/// `shipped_files` is an optional read-only directory holding the files an
/// image ships under `files/` (CSS, JS, fonts, downloads). `/files/*` reads
/// `paths.storage/files` first and falls back to it, so `paths.storage` only
/// has to hold user data and nothing has to be copied onto it at boot.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PathsConfig {
    pub system: String,
    pub services: String,
    pub bin: String,

    #[serde(default)]
    pub web_path: Option<String>,

    #[serde(default)]
    pub storage: Option<String>,

    #[serde(default)]
    pub geoip_database: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shipped_files: Option<String>,

    #[serde(default)]
    pub data: Option<String>,
}

impl PathsConfig {
    pub fn resolve_relative_to(&mut self, base: &Path) {
        self.system = resolve_path(base, &self.system);
        self.services = resolve_path(base, &self.services);
        self.bin = resolve_path(base, &self.bin);
        self.storage = self.storage.as_ref().map(|p| resolve_path(base, p));
        self.geoip_database = self.geoip_database.as_ref().map(|p| resolve_path(base, p));
        self.web_path = self.web_path.as_ref().map(|p| resolve_path(base, p));
        self.shipped_files = self.shipped_files.as_ref().map(|p| resolve_path(base, p));
        self.data = self.data.as_ref().map(|p| resolve_path(base, p));
    }

    #[must_use]
    pub fn with_services_root(&self, root: &Path) -> Self {
        Self {
            services: root.to_string_lossy().into_owned(),
            ..self.clone()
        }
    }

    pub fn skills(&self) -> String {
        format!("{}/skills", self.services)
    }

    pub fn config(&self) -> String {
        format!("{}/config/config.yaml", self.services)
    }

    pub fn ai_config(&self) -> String {
        format!("{}/ai/config.yaml", self.services)
    }

    pub fn content_config(&self) -> String {
        format!("{}/content/config.yaml", self.services)
    }

    pub fn web_config(&self) -> String {
        format!("{}/web/config.yaml", self.services)
    }

    pub fn web_metadata(&self) -> String {
        format!("{}/web/metadata.yaml", self.services)
    }

    pub fn plugins(&self) -> String {
        format!("{}/plugins", self.services)
    }

    pub fn marketplaces(&self) -> String {
        format!("{}/marketplaces", self.services)
    }

    pub fn hooks(&self) -> String {
        format!("{}/hooks", self.services)
    }

    pub fn agents(&self) -> String {
        format!("{}/agents", self.services)
    }

    pub fn logs(&self) -> String {
        format!("{}/logs", self.system)
    }

    pub fn web_path_resolved(&self) -> String {
        self.web_path
            .clone()
            .unwrap_or_else(|| format!("{}/web", self.system))
    }

    pub fn storage_resolved(&self) -> Option<&str> {
        self.storage.as_deref()
    }

    pub fn geoip_database_resolved(&self) -> Option<&str> {
        self.geoip_database.as_deref()
    }
}

pub fn resolve_path(base: &Path, path: &str) -> String {
    let p = Path::new(path);
    if p.is_absolute() {
        path.to_owned()
    } else {
        let resolved = base.join(p);
        resolved.canonicalize().map_or_else(
            |_| resolved.to_string_lossy().to_string(),
            |canonical| canonical.to_string_lossy().to_string(),
        )
    }
}

pub fn expand_home(path_str: &str) -> PathBuf {
    path_str.strip_prefix("~/").map_or_else(
        || PathBuf::from(path_str),
        |stripped| {
            let home = std::env::var("HOME")
                .or_else(|_| std::env::var("USERPROFILE"))
                .unwrap_or_else(|_| {
                    tracing::warn!(
                        path = %path_str,
                        "Cannot expand ~/ path: neither HOME nor USERPROFILE is set"
                    );
                    String::new()
                });
            PathBuf::from(home).join(stripped)
        },
    )
}

pub fn resolve_with_home(base: &Path, path_str: &str) -> PathBuf {
    let path = expand_home(path_str);

    if path.is_absolute() {
        path
    } else {
        base.join(path)
    }
}
