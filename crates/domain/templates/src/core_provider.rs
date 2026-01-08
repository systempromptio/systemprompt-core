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

#[derive(Debug)]
pub struct CoreTemplateProvider {
    template_dir: PathBuf,
    templates: Vec<TemplateDefinition>,
}

impl CoreTemplateProvider {
    #[must_use]
    pub fn new(template_dir: impl Into<PathBuf>) -> Self {
        Self {
            template_dir: template_dir.into(),
            templates: Vec::new(),
        }
    }

    pub async fn discover(&mut self) -> anyhow::Result<()> {
        self.templates = discover_templates(&self.template_dir).await?;
        Ok(())
    }

    pub async fn discover_from(template_dir: impl Into<PathBuf>) -> anyhow::Result<Self> {
        let mut provider = Self::new(template_dir);
        provider.discover().await?;
        Ok(provider)
    }
}

impl TemplateProvider for CoreTemplateProvider {
    fn provider_id(&self) -> &'static str {
        "core"
    }

    fn priority(&self) -> u32 {
        1000
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

async fn discover_templates(dir: &Path) -> anyhow::Result<Vec<TemplateDefinition>> {
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
                priority: 1000,
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
