use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathsConfig {
    pub system: String,

    pub core: String,

    pub services: String,

    #[serde(default)]
    pub skills: Option<String>,

    #[serde(default)]
    pub config: Option<String>,

    #[serde(default)]
    pub storage: Option<String>,

    #[serde(default)]
    pub geoip_database: Option<String>,

    #[serde(default)]
    pub ai_config: Option<String>,

    #[serde(default)]
    pub content_config: Option<String>,

    #[serde(default)]
    pub web_config: Option<String>,

    #[serde(default)]
    pub web_metadata: Option<String>,

    #[serde(default)]
    pub web_path: Option<String>,

    #[serde(default)]
    pub scg_templates: Option<String>,

    #[serde(default)]
    pub scg_assets: Option<String>,
}

impl PathsConfig {
    pub fn resolve_relative_to(&mut self, base: &Path) {
        self.system = resolve_path(base, &self.system);
        self.core = resolve_path(base, &self.core);
        self.services = resolve_path(base, &self.services);
        self.skills = self.skills.as_ref().map(|p| resolve_path(base, p));
        self.config = self.config.as_ref().map(|p| resolve_path(base, p));
        self.storage = self.storage.as_ref().map(|p| resolve_path(base, p));
        self.geoip_database = self.geoip_database.as_ref().map(|p| resolve_path(base, p));
        self.ai_config = self.ai_config.as_ref().map(|p| resolve_path(base, p));
        self.content_config = self.content_config.as_ref().map(|p| resolve_path(base, p));
        self.web_config = self.web_config.as_ref().map(|p| resolve_path(base, p));
        self.web_metadata = self.web_metadata.as_ref().map(|p| resolve_path(base, p));
        self.web_path = self.web_path.as_ref().map(|p| resolve_path(base, p));
        self.scg_templates = self.scg_templates.as_ref().map(|p| resolve_path(base, p));
        self.scg_assets = self.scg_assets.as_ref().map(|p| resolve_path(base, p));
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
