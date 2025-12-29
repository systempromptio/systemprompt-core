use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct IncludeResolver {
    base_path: PathBuf,
}

impl IncludeResolver {
    pub const fn new(base_path: PathBuf) -> Self {
        Self { base_path }
    }

    pub fn resolve_string(&self, value: &str) -> Result<String> {
        value.strip_prefix("!include ").map_or_else(
            || Ok(value.to_string()),
            |include_path| {
                let full_path = self.base_path.join(include_path.trim());
                fs::read_to_string(&full_path)
                    .with_context(|| format!("Failed to read include: {}", full_path.display()))
            },
        )
    }

    pub fn resolve_yaml_file<T: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<T> {
        let full_path = self.base_path.join(path);
        let content = fs::read_to_string(&full_path)
            .with_context(|| format!("Failed to read YAML: {}", full_path.display()))?;

        serde_yaml::from_str(&content)
            .with_context(|| format!("Failed to parse YAML: {}", full_path.display()))
    }

    pub fn with_subpath(&self, subpath: &str) -> Self {
        Self {
            base_path: self.base_path.join(subpath),
        }
    }

    pub fn base_path(&self) -> &Path {
        &self.base_path
    }

    pub fn exists(&self, path: &str) -> bool {
        self.base_path.join(path).exists()
    }

    pub fn read_file(&self, path: &str) -> Result<String> {
        let full_path = self.base_path.join(path);
        fs::read_to_string(&full_path)
            .with_context(|| format!("Failed to read file: {}", full_path.display()))
    }
}
