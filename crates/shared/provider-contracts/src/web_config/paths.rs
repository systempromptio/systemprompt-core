use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathsConfig {
    pub templates: String,
    pub assets: String,
    #[serde(default)]
    pub css_url_prefix: Option<String>,
}

impl PathsConfig {
    pub fn resolve_relative_to(&mut self, base: &Path) {
        self.templates = resolve_path(base, &self.templates);
    }
}

fn resolve_path(base: &Path, path: &str) -> String {
    if path.is_empty() {
        return path.to_string();
    }

    let p = Path::new(path);
    if p.is_absolute() {
        return path.to_string();
    }

    let resolved = base.join(p);
    resolved.canonicalize().map_or_else(
        |_| resolved.to_string_lossy().to_string(),
        |canonical| canonical.to_string_lossy().to_string(),
    )
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptConfig {
    pub src: String,
    #[serde(default)]
    pub defer: bool,
    #[serde(default)]
    pub r#async: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentConfig {
    #[serde(default)]
    pub config_file: Option<String>,
    #[serde(default)]
    pub sources: Vec<String>,
}
