use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use systemprompt_models::Secrets;

#[derive(Debug, Clone, Copy)]
pub struct SecretsLoader;

impl SecretsLoader {
    pub fn load_from_file(path: &Path) -> Result<Secrets> {
        if !path.exists() {
            anyhow::bail!("Secrets file not found: {}", path.display());
        }

        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read secrets file: {}", path.display()))?;

        Secrets::parse(&content)
            .with_context(|| format!("Failed to parse secrets file: {}", path.display()))
    }

    pub fn resolve_and_load(path_str: &str, base_dir: Option<&Path>) -> Result<Secrets> {
        let path = Self::resolve_path(path_str, base_dir)?;
        Self::load_from_file(&path)
    }

    pub fn resolve_path(path_str: &str, base_dir: Option<&Path>) -> Result<PathBuf> {
        let path = if let Some(stripped) = path_str.strip_prefix("~/") {
            let home = std::env::var("HOME")
                .or_else(|_| std::env::var("USERPROFILE"))
                .map_err(|_| {
                    anyhow::anyhow!(
                        "Cannot resolve path '{}': neither HOME nor USERPROFILE environment \
                         variable is set",
                        path_str
                    )
                })?;
            PathBuf::from(home).join(stripped)
        } else {
            PathBuf::from(path_str)
        };

        if path.is_relative() {
            Ok(base_dir.map_or_else(|| path.clone(), |base| base.join(&path)))
        } else {
            Ok(path)
        }
    }

    pub fn exists(path: &Path) -> bool {
        path.exists()
    }
}
