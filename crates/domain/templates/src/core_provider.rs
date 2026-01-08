use std::path::{Path, PathBuf};

use systemprompt_template_provider::{TemplateDefinition, TemplateProvider, TemplateSource};
use tokio::fs;
use tracing::debug;

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

async fn discover_templates(dir: &Path) -> anyhow::Result<Vec<TemplateDefinition>> {
    let mut templates = Vec::new();

    if !dir.exists() {
        return Ok(templates);
    }

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

            let content_types = infer_content_types(&template_name);

            templates.push(TemplateDefinition {
                name: template_name,
                source: TemplateSource::File(path),
                priority: 1000,
                content_types,
            });
        }
    }

    Ok(templates)
}

fn infer_content_types(name: &str) -> Vec<String> {
    match name {
        "paper" => vec!["paper".into()],
        "blog-post" => vec!["blog".into()],
        "docs-post" => vec!["docs".into()],
        "paper-list" => vec!["paper-list".into()],
        "blog-list" => vec!["blog-list".into()],
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
