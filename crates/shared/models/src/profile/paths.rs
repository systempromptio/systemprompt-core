use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
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
}

impl PathsConfig {
    pub fn resolve_relative_to(&mut self, base: &Path) {
        self.system = resolve_path(base, &self.system);
        self.services = resolve_path(base, &self.services);
        self.bin = resolve_path(base, &self.bin);
        self.storage = self.storage.as_ref().map(|p| resolve_path(base, p));
        self.geoip_database = self.geoip_database.as_ref().map(|p| resolve_path(base, p));
        self.web_path = self.web_path.as_ref().map(|p| resolve_path(base, p));
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
        path.to_string()
    } else {
        let resolved = base.join(p);
        resolved
            .canonicalize()
            .map(|canonical| canonical.to_string_lossy().to_string())
            .unwrap_or_else(|_| resolved.to_string_lossy().to_string())
    }
}

pub fn expand_home(path_str: &str) -> PathBuf {
    path_str.strip_prefix("~/").map_or_else(
        || PathBuf::from(path_str),
        |stripped| {
            let home = std::env::var("HOME")
                .or_else(|_| std::env::var("USERPROFILE"))
                .unwrap_or_default();
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
