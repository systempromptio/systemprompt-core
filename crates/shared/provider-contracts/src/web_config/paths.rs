//! Filesystem-path configuration for templates, assets, and content.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// Filesystem paths driving template / asset discovery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathsConfig {
    /// Path to the template directory.
    pub templates: String,
    /// Path to the static-assets directory.
    pub assets: String,
    /// Optional URL prefix prepended to emitted CSS / asset URLs.
    #[serde(default)]
    pub css_url_prefix: Option<String>,
}

impl PathsConfig {
    /// Resolve relative paths in this config against `base`.
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

/// Declaration of one `<script>` tag injected into rendered pages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptConfig {
    /// `src` attribute of the script tag.
    pub src: String,
    /// Whether to emit the `defer` attribute.
    #[serde(default)]
    pub defer: bool,
    /// Whether to emit the `async` attribute.
    #[serde(default)]
    pub r#async: bool,
}

/// Optional content-source registration block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentConfig {
    /// Path to a separate content-config YAML, when split out.
    #[serde(default)]
    pub config_file: Option<String>,
    /// Inline content-source identifiers.
    #[serde(default)]
    pub sources: Vec<String>,
}
