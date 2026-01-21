//! Core template provider implementation.
//!
//! This module provides [`CoreTemplateProvider`], which discovers HTML templates
//! from a filesystem directory and optionally reads metadata from a `templates.yaml` manifest.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use systemprompt_template_provider::{TemplateDefinition, TemplateProvider, TemplateSource};
use tokio::fs;
use tracing::{debug, warn};

#[derive(Debug, Deserialize, Default)]
struct TemplateManifest {
    #[serde(default)]
    templates: HashMap<String, TemplateConfig>,
}

#[derive(Debug, Deserialize)]
struct TemplateConfig {
    #[serde(default)]
    content_types: Vec<String>,
}

/// A template provider that discovers HTML templates from a filesystem directory.
///
/// Templates are discovered by scanning for `.html` files in the specified directory.
/// Optional metadata can be provided via a `templates.yaml` manifest file.
///
/// # Priority
///
/// Templates have a priority that determines resolution order when multiple providers
/// register templates with the same name. Lower priority values win (are resolved first).
///
/// - [`DEFAULT_PRIORITY`](Self::DEFAULT_PRIORITY) (1000): Standard templates
/// - [`EXTENSION_PRIORITY`](Self::EXTENSION_PRIORITY) (500): Extension templates that override defaults
#[derive(Debug)]
pub struct CoreTemplateProvider {
    template_dir: PathBuf,
    templates: Vec<TemplateDefinition>,
    priority: u32,
}

impl CoreTemplateProvider {
    /// Default priority for standard templates.
    pub const DEFAULT_PRIORITY: u32 = 1000;

    /// Priority for extension templates that should override defaults.
    pub const EXTENSION_PRIORITY: u32 = 500;

    /// Creates a new provider for the given template directory with default priority.
    #[must_use]
    pub fn new(template_dir: impl Into<PathBuf>) -> Self {
        Self {
            template_dir: template_dir.into(),
            templates: Vec::new(),
            priority: Self::DEFAULT_PRIORITY,
        }
    }

    /// Creates a new provider with a custom priority.
    #[must_use]
    pub fn with_priority(template_dir: impl Into<PathBuf>, priority: u32) -> Self {
        Self {
            template_dir: template_dir.into(),
            templates: Vec::new(),
            priority,
        }
    }

    /// Discovers templates in the configured directory.
    ///
    /// This scans the directory for `.html` files and reads any `templates.yaml` manifest.
    pub async fn discover(&mut self) -> anyhow::Result<()> {
        self.templates = discover_templates(&self.template_dir, self.priority).await?;
        Ok(())
    }

    /// Creates a new provider and immediately discovers templates.
    pub async fn discover_from(template_dir: impl Into<PathBuf>) -> anyhow::Result<Self> {
        let mut provider = Self::new(template_dir);
        provider.discover().await?;
        Ok(provider)
    }

    /// Creates a new provider with custom priority and immediately discovers templates.
    pub async fn discover_with_priority(
        template_dir: impl Into<PathBuf>,
        priority: u32,
    ) -> anyhow::Result<Self> {
        let mut provider = Self::with_priority(template_dir, priority);
        provider.discover().await?;
        Ok(provider)
    }
}

impl TemplateProvider for CoreTemplateProvider {
    fn provider_id(&self) -> &'static str {
        "core"
    }

    fn priority(&self) -> u32 {
        self.priority
    }

    fn templates(&self) -> Vec<TemplateDefinition> {
        self.templates.clone()
    }
}

async fn load_manifest(dir: &Path) -> TemplateManifest {
    let manifest_path = dir.join("templates.yaml");

    let Ok(content) = fs::read_to_string(&manifest_path).await else {
        return TemplateManifest::default();
    };

    match serde_yaml::from_str(&content) {
        Ok(manifest) => {
            debug!(path = %manifest_path.display(), "Loaded template manifest");
            manifest
        },
        Err(e) => {
            warn!(
                path = %manifest_path.display(),
                error = %e,
                "Failed to parse template manifest, using defaults"
            );
            TemplateManifest::default()
        },
    }
}

async fn discover_templates(dir: &Path, priority: u32) -> anyhow::Result<Vec<TemplateDefinition>> {
    let mut templates = Vec::new();

    if !dir.exists() {
        return Ok(templates);
    }

    let manifest = load_manifest(dir).await;
    let mut entries = fs::read_dir(dir).await?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();

        if path.extension().is_some_and(|ext| ext == "html") {
            let Some(file_stem) = path.file_stem() else {
                continue;
            };
            let template_name = file_stem.to_string_lossy().to_string();

            debug!(
                template = %template_name,
                path = %path.display(),
                priority = priority,
                "Discovered template"
            );

            let content_types = manifest.templates.get(&template_name).map_or_else(
                || infer_content_types(&template_name),
                |config| config.content_types.clone(),
            );

            let filename = path.file_name().map_or_else(|| path.clone(), PathBuf::from);

            templates.push(TemplateDefinition {
                name: template_name,
                source: TemplateSource::File(filename),
                priority,
                content_types,
            });
        }
    }

    Ok(templates)
}

fn infer_content_types(name: &str) -> Vec<String> {
    match name {
        _ if name.ends_with("-post") => {
            let content_type = name.trim_end_matches("-post");
            vec![content_type.into()]
        },
        _ if name.ends_with("-list") => {
            vec![name.into()]
        },
        _ => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_creates_provider_with_default_priority() {
        let provider = CoreTemplateProvider::new("/tmp/templates");
        assert_eq!(provider.priority, CoreTemplateProvider::DEFAULT_PRIORITY);
        assert!(provider.templates.is_empty());
    }

    #[test]
    fn test_with_priority_creates_provider_with_custom_priority() {
        let provider = CoreTemplateProvider::with_priority("/tmp/templates", 500);
        assert_eq!(provider.priority, 500);
    }

    #[test]
    fn test_provider_id_returns_core() {
        let provider = CoreTemplateProvider::new("/tmp/templates");
        assert_eq!(provider.provider_id(), "core");
    }

    #[test]
    fn test_infer_content_types_post_suffix() {
        let types = infer_content_types("article-post");
        assert_eq!(types, vec!["article"]);
    }

    #[test]
    fn test_infer_content_types_list_suffix() {
        let types = infer_content_types("articles-list");
        assert_eq!(types, vec!["articles-list"]);
    }

    #[test]
    fn test_infer_content_types_no_suffix() {
        let types = infer_content_types("base");
        assert!(types.is_empty());
    }

    #[tokio::test]
    async fn test_discover_from_nonexistent_directory() {
        let provider = CoreTemplateProvider::discover_from("/nonexistent/path").await;
        assert!(provider.is_ok());
        assert!(provider.unwrap().templates().is_empty());
    }

    #[tokio::test]
    async fn test_discover_templates_from_temp_directory() {
        let temp_dir = tempfile::tempdir().unwrap();
        let template_path = temp_dir.path().join("test-post.html");
        fs::write(&template_path, "<html></html>").await.unwrap();

        let provider = CoreTemplateProvider::discover_from(temp_dir.path()).await.unwrap();
        let templates = provider.templates();

        assert_eq!(templates.len(), 1);
        assert_eq!(templates[0].name, "test-post");
        assert_eq!(templates[0].content_types, vec!["test"]);
    }

    #[tokio::test]
    async fn test_discover_with_manifest() {
        let temp_dir = tempfile::tempdir().unwrap();

        // Create template file
        let template_path = temp_dir.path().join("custom.html");
        fs::write(&template_path, "<html></html>").await.unwrap();

        // Create manifest with custom content types
        let manifest = r#"
templates:
  custom:
    content_types:
      - page
      - article
"#;
        let manifest_path = temp_dir.path().join("templates.yaml");
        fs::write(&manifest_path, manifest).await.unwrap();

        let provider = CoreTemplateProvider::discover_from(temp_dir.path()).await.unwrap();
        let templates = provider.templates();

        assert_eq!(templates.len(), 1);
        assert_eq!(templates[0].name, "custom");
        assert_eq!(templates[0].content_types, vec!["page", "article"]);
    }
}
