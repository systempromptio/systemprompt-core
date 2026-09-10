//! `skills/<id>/SKILL.md` → `skills/<id>/config.yaml` plus a verbatim copy.
//!
//! The skill id is the directory name, never the frontmatter `name`: the loader
//! keys skills by directory and rejects a descriptor whose id disagrees with
//! it, and Anthropic's `name` is a display string that may contain anything.
//!
//! Skill ids are canonically `snake_case`, while Claude Code's bundle layout
//! names the directory in `kebab-case`. A snake id contains no hyphens, so
//! mapping `-` to `_` inverts that projection exactly and a bundle generated
//! from a services tree imports back to the ids it started with.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use systemprompt_models::services::frontmatter::split_frontmatter;

use crate::error::MarketplaceError;

use super::disk::SkillDoc;
use super::writer::Sink;

pub(super) const SKILL_FILE: &str = "SKILL.md";

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum TagList {
    One(String),
    Many(Vec<String>),
}

impl TagList {
    fn into_vec(self) -> Vec<String> {
        match self {
            Self::One(s) => s
                .split(',')
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .map(str::to_owned)
                .collect(),
            Self::Many(v) => v,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
struct SkillFrontmatter {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    tags: Option<TagList>,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    hosts: Vec<String>,
}

pub(super) fn discover_skill_dirs(plugin_dir: &Path) -> Vec<(String, PathBuf)> {
    let skills_dir = plugin_dir.join("skills");
    let Ok(read) = std::fs::read_dir(&skills_dir) else {
        return Vec::new();
    };
    let mut out: Vec<(String, PathBuf)> = read
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_dir() && p.join(SKILL_FILE).is_file())
        .filter_map(|p| {
            let name = p.file_name()?.to_str()?.replace('-', "_");
            Some((name, p))
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

pub(super) fn import_skill(
    id: &str,
    skill_dir: &Path,
    fallback_category: Option<&str>,
    sink: &Sink,
) -> Result<(), MarketplaceError> {
    let skill_md = skill_dir.join(SKILL_FILE);
    let raw = std::fs::read_to_string(&skill_md).map_err(|e| MarketplaceError::Import {
        path: skill_md.display().to_string(),
        message: e.to_string(),
    })?;

    let front: SkillFrontmatter = match split_frontmatter(&raw) {
        Some(f) => serde_yaml::from_str(f.yaml).map_err(|e| MarketplaceError::Import {
            path: skill_md.display().to_string(),
            message: format!("SKILL.md frontmatter is not valid YAML: {e}"),
        })?,
        None => SkillFrontmatter::default(),
    };

    let description = front.description.unwrap_or_default();
    if description.trim().is_empty() {
        return Err(MarketplaceError::Import {
            path: skill_md.display().to_string(),
            message: "SKILL.md frontmatter must set a non-empty 'description'".to_owned(),
        });
    }

    let doc = SkillDoc {
        id: id.to_owned(),
        name: front
            .name
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| id.replace(['_', '-'], " ")),
        description,
        enabled: true,
        file: SKILL_FILE.to_owned(),
        tags: front.tags.map(TagList::into_vec).unwrap_or_default(),
        category: front
            .category
            .or_else(|| fallback_category.map(str::to_owned)),
        hosts: front.hosts,
    };

    let rel = Path::new("skills").join(id);
    sink.copy_tree(skill_dir, &rel)?;
    sink.write_yaml(&rel.join("config.yaml"), &doc)
}
