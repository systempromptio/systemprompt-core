use std::path::Path;

use sha2::{Digest, Sha256};
use systemprompt_models::bridge::ids::{Sha256Digest, SkillId, SkillName};
use systemprompt_models::bridge::manifest::SkillEntry;
use systemprompt_models::services::{DiskSkillConfig, SKILL_CONFIG_FILENAME, strip_frontmatter};

use crate::error::MarketplaceError;

pub fn load_skills(services_root: &Path) -> Result<Vec<SkillEntry>, MarketplaceError> {
    let skills_dir = services_root.join("skills");
    if !skills_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut entries: Vec<(String, std::path::PathBuf)> = Vec::new();
    let read = std::fs::read_dir(&skills_dir).map_err(|e| MarketplaceError::Catalog(e.to_string()))?;
    for entry in read {
        let entry = entry.map_err(|e| MarketplaceError::Catalog(e.to_string()))?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(dir_name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let config_path = path.join(SKILL_CONFIG_FILENAME);
        if !config_path.exists() {
            continue;
        }
        entries.push((dir_name.to_owned(), path));
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let mut out = Vec::with_capacity(entries.len());
    for (dir_name, skill_dir) in entries {
        match build_skill_entry(&dir_name, &skill_dir) {
            Ok(Some(entry)) => out.push(entry),
            Ok(None) => {},
            Err(e) => {
                tracing::warn!(
                    skill_dir = %skill_dir.display(),
                    error = %e,
                    "manifest: failed to build skill entry; skipping"
                );
            },
        }
    }
    Ok(out)
}

fn build_skill_entry(
    dir_name: &str,
    skill_dir: &Path,
) -> Result<Option<SkillEntry>, MarketplaceError> {
    let config_path = skill_dir.join(SKILL_CONFIG_FILENAME);
    let config_text =
        std::fs::read_to_string(&config_path).map_err(|e| MarketplaceError::Catalog(e.to_string()))?;
    let config: DiskSkillConfig = serde_yaml::from_str(&config_text)
        .map_err(|e| MarketplaceError::Catalog(format!("parse {}: {e}", config_path.display())))?;

    if !config.enabled {
        return Ok(None);
    }

    let id = if config.id.as_str().is_empty() {
        SkillId::try_new(dir_name.replace('-', "_"))
            .map_err(|e| MarketplaceError::Catalog(e.to_string()))?
    } else {
        SkillId::try_new(config.id.as_str()).map_err(|e| MarketplaceError::Catalog(e.to_string()))?
    };
    let display_name = if config.name.is_empty() {
        dir_name.replace('_', " ")
    } else {
        config.name.clone()
    };
    let name = SkillName::try_new(display_name).map_err(|e| MarketplaceError::Catalog(e.to_string()))?;

    let content_path = skill_dir.join(config.content_file());
    let instructions = if content_path.exists() {
        let raw = std::fs::read_to_string(&content_path)
            .map_err(|e| MarketplaceError::Catalog(e.to_string()))?;
        strip_frontmatter(&raw)
    } else {
        String::new()
    };

    let mut hasher = Sha256::new();
    hasher.update(instructions.as_bytes());
    let sha256 = Sha256Digest::try_new(hex::encode(hasher.finalize()))
        .map_err(|e| MarketplaceError::Catalog(e.to_string()))?;

    Ok(Some(SkillEntry {
        id,
        name,
        description: config.description,
        file_path: content_path.to_string_lossy().into_owned(),
        tags: config.tags,
        sha256,
        instructions,
    }))
}
