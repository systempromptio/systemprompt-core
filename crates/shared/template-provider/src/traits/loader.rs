use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

use async_trait::async_trait;
use systemprompt_provider_contracts::TemplateSource;
use tokio::fs;

use super::error::{Result, TemplateLoaderError};

#[async_trait]
pub trait TemplateLoader: Send + Sync {
    async fn load(&self, source: &TemplateSource) -> Result<String>;

    fn can_load(&self, source: &TemplateSource) -> bool;

    async fn load_directory(&self, _path: &Path) -> Result<Vec<(String, String)>> {
        Err(TemplateLoaderError::DirectoryLoadingUnsupported)
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

    fn has_traversal_components(path: &Path) -> bool {
        path.components().any(|c| matches!(c, Component::ParentDir))
    }

    async fn is_within_base_paths(&self, canonical: &Path) -> Result<bool> {
        for base in &self.base_paths {
            match fs::canonicalize(base).await {
                Ok(canonical_base) if canonical.starts_with(&canonical_base) => return Ok(true),
                Ok(_) => {},
                Err(e) if e.kind() == ErrorKind::NotFound => {},
                Err(e) => return Err(TemplateLoaderError::io(base, e)),
            }
        }
        Ok(false)
    }

    async fn canonicalize_and_validate(&self, path: &Path) -> Result<PathBuf> {
        let canonical = fs::canonicalize(path)
            .await
            .map_err(|e| TemplateLoaderError::io(path, e))?;

        if !self.is_within_base_paths(&canonical).await? {
            return Err(TemplateLoaderError::OutsideBasePath(path.to_path_buf()));
        }

        Ok(canonical)
    }

    async fn try_read_from_base(&self, base: &Path, relative: &Path) -> Option<Result<String>> {
        let full_path = base.join(relative);

        match fs::canonicalize(&full_path).await {
            Ok(canonical) => {
                let canonical_base = match fs::canonicalize(base).await {
                    Ok(cb) => cb,
                    Err(e) => return Some(Err(TemplateLoaderError::io(base, e))),
                };

                if !canonical.starts_with(&canonical_base) {
                    return Some(Err(TemplateLoaderError::OutsideBasePath(full_path)));
                }

                Some(
                    fs::read_to_string(&canonical)
                        .await
                        .map_err(|e| TemplateLoaderError::io(&full_path, e)),
                )
            },
            Err(e) if e.kind() == ErrorKind::NotFound => None,
            Err(e) => Some(Err(TemplateLoaderError::io(&full_path, e))),
        }
    }
}

#[async_trait]
impl TemplateLoader for FileSystemLoader {
    async fn load(&self, source: &TemplateSource) -> Result<String> {
        match source {
            TemplateSource::Embedded(content) => Ok((*content).to_string()),
            TemplateSource::File(path) => {
                if Self::has_traversal_components(path) {
                    return Err(TemplateLoaderError::DirectoryTraversal(path.clone()));
                }

                if path.is_absolute() {
                    let canonical = self.canonicalize_and_validate(path).await?;
                    return fs::read_to_string(&canonical)
                        .await
                        .map_err(|e| TemplateLoaderError::io(path, e));
                }

                if self.base_paths.is_empty() {
                    return Err(TemplateLoaderError::NoBasePaths);
                }

                for base in &self.base_paths {
                    if let Some(result) = self.try_read_from_base(base, path).await {
                        return result;
                    }
                }

                Err(TemplateLoaderError::NotFound(path.clone()))
            },
            TemplateSource::Directory(path) => {
                Err(TemplateLoaderError::DirectoryNotSupported(path.clone()))
            },
        }
    }

    fn can_load(&self, source: &TemplateSource) -> bool {
        matches!(
            source,
            TemplateSource::Embedded(_) | TemplateSource::File(_)
        )
    }

    async fn load_directory(&self, path: &Path) -> Result<Vec<(String, String)>> {
        if Self::has_traversal_components(path) {
            return Err(TemplateLoaderError::DirectoryTraversal(path.to_path_buf()));
        }

        if self.base_paths.is_empty() {
            return Err(TemplateLoaderError::NoBasePaths);
        }

        let dir_path = if path.is_absolute() {
            self.canonicalize_and_validate(path).await?
        } else {
            let mut found_path = None;
            for base in &self.base_paths {
                let candidate = base.join(path);
                match fs::canonicalize(&candidate).await {
                    Ok(canonical) => {
                        let canonical_base = fs::canonicalize(base)
                            .await
                            .map_err(|e| TemplateLoaderError::io(base, e))?;

                        if !canonical.starts_with(&canonical_base) {
                            return Err(TemplateLoaderError::OutsideBasePath(candidate));
                        }

                        found_path = Some(canonical);
                        break;
                    },
                    Err(e) if e.kind() == ErrorKind::NotFound => {},
                    Err(e) => return Err(TemplateLoaderError::io(&candidate, e)),
                }
            }
            found_path.ok_or_else(|| TemplateLoaderError::NotFound(path.to_path_buf()))?
        };

        let mut templates = Vec::new();
        let mut entries = fs::read_dir(&dir_path)
            .await
            .map_err(|e| TemplateLoaderError::io(&dir_path, e))?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| TemplateLoaderError::io(&dir_path, e))?
        {
            let entry_path = entry.path();

            if entry_path.extension().is_some_and(|ext| ext == "html") {
                let Some(file_stem) = entry_path.file_stem() else {
                    continue;
                };

                let template_name = file_stem
                    .to_str()
                    .ok_or_else(|| TemplateLoaderError::InvalidEncoding(entry_path.clone()))?
                    .to_owned();

                let content = fs::read_to_string(&entry_path)
                    .await
                    .map_err(|e| TemplateLoaderError::io(&entry_path, e))?;

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
            _ => Err(TemplateLoaderError::EmbeddedOnly),
        }
    }

    fn can_load(&self, source: &TemplateSource) -> bool {
        matches!(source, TemplateSource::Embedded(_))
    }
}
