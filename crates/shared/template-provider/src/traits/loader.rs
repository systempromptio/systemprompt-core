use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use systemprompt_provider_contracts::TemplateSource;
use tokio::fs;

#[async_trait]
pub trait TemplateLoader: Send + Sync {
    async fn load(&self, source: &TemplateSource) -> Result<String>;

    fn can_load(&self, source: &TemplateSource) -> bool;

    async fn load_directory(&self, path: &Path) -> Result<Vec<(String, String)>> {
        let _ = path;
        Err(anyhow!("Directory loading not supported by this loader"))
    }
}

#[derive(Debug, Default)]
pub struct FileSystemLoader {
    base_paths: Vec<PathBuf>,
}

impl FileSystemLoader {
    #[must_use]
    pub const fn new(base_paths: Vec<PathBuf>) -> Self {
        Self { base_paths }
    }

    #[must_use]
    pub fn with_path(path: impl Into<PathBuf>) -> Self {
        Self {
            base_paths: vec![path.into()],
        }
    }

    #[must_use]
    pub fn add_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.base_paths.push(path.into());
        self
    }
}

#[async_trait]
impl TemplateLoader for FileSystemLoader {
    async fn load(&self, source: &TemplateSource) -> Result<String> {
        match source {
            TemplateSource::Embedded(content) => Ok((*content).to_string()),
            TemplateSource::File(path) => {
                if path.is_absolute() && path.exists() {
                    return fs::read_to_string(path)
                        .await
                        .map_err(|e| anyhow!("Failed to load template {}: {}", path.display(), e));
                }

                for base in &self.base_paths {
                    let full_path = base.join(path);
                    if full_path.exists() {
                        return fs::read_to_string(&full_path).await.map_err(|e| {
                            anyhow!("Failed to load template {}: {}", full_path.display(), e)
                        });
                    }
                }

                Err(anyhow!("Template not found: {}", path.display()))
            },
            TemplateSource::Directory(path) => Err(anyhow!(
                "Cannot load single template from directory: {}",
                path.display()
            )),
        }
    }

    fn can_load(&self, source: &TemplateSource) -> bool {
        matches!(
            source,
            TemplateSource::Embedded(_) | TemplateSource::File(_) | TemplateSource::Directory(_)
        )
    }

    async fn load_directory(&self, path: &Path) -> Result<Vec<(String, String)>> {
        let mut templates = Vec::new();
        let mut seen = HashSet::new();

        let dir_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.base_paths
                .iter()
                .map(|base| base.join(path))
                .find(|p| p.exists())
                .ok_or_else(|| anyhow!("Template directory not found: {}", path.display()))?
        };

        if !dir_path.exists() {
            return Err(anyhow!(
                "Template directory not found: {}",
                dir_path.display()
            ));
        }

        let mut entries = fs::read_dir(&dir_path).await?;

        while let Some(entry) = entries.next_entry().await? {
            let entry_path = entry.path();

            if entry_path.extension().is_some_and(|ext| ext == "html") {
                let Some(file_stem) = entry_path.file_stem() else {
                    continue;
                };
                let template_name = file_stem.to_string_lossy().to_string();

                if seen.contains(&template_name) {
                    continue;
                }

                let content = fs::read_to_string(&entry_path).await?;
                seen.insert(template_name.clone());
                templates.push((template_name, content));
            }
        }

        Ok(templates)
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct EmbeddedLoader;

#[async_trait]
impl TemplateLoader for EmbeddedLoader {
    async fn load(&self, source: &TemplateSource) -> Result<String> {
        match source {
            TemplateSource::Embedded(content) => Ok((*content).to_string()),
            _ => Err(anyhow!("EmbeddedLoader only handles embedded templates")),
        }
    }

    fn can_load(&self, source: &TemplateSource) -> bool {
        matches!(source, TemplateSource::Embedded(_))
    }
}
