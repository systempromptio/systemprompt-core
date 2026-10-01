//! `skills/<id>/SKILL.md` → `skills/<id>/config.yaml` plus a verbatim copy.
//!
//! The skill id is the directory name, never the frontmatter `name`: the loader
//! keys skills by directory and rejects a descriptor whose id disagrees with
//! it, and Anthropic's `name` is a display string that may contain anything.
//!
//! The display name is the frontmatter `title` when present, else `name`:
//! Claude Code constrains `name` to a kebab-case slug and ignores keys it does
//! not know, so `title` and `display_category` ride in the same frontmatter.
//!
//! Skill ids are canonically `snake_case`, while Claude Code's bundle layout
//! names the directory in `kebab-case`. A snake id contains no hyphens, so
//! mapping `-` to `_` inverts that projection exactly and a bundle generated
//! from a services tree imports back to the ids it started with.
//!
//! A plugin's skills are its `skills/` folders plus each path its manifest
//! names under Claude Code's `skills` override; a named path is a skill itself
//! or a folder of them, so `"./"` reads skill folders at the plugin root.
//!
//! The skill folder is copied without its dev-only files (see
//! [`crate::dev_files`]).
//!
//! Every frontmatter key the platform does not own (see
//! `systemprompt_models::services::skill_frontmatter`) is kept in `config.yaml`
//! under `frontmatter`, in authored order, and rendered back into the client
//! `SKILL.md`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_yaml::Value;
use systemprompt_models::services::skill_frontmatter::{
    SplitSkillFrontmatter, authored_skill_frontmatter, check_json_compatible,
    split_skill_frontmatter,
};

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
    title: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    tags: Option<TagList>,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    display_category: Option<String>,
    #[serde(default)]
    hosts: Vec<String>,
}

pub(super) fn discover_skill_dirs(plugin_dir: &Path, extra: &[PathBuf]) -> Vec<(String, PathBuf)> {
    let mut out: Vec<(String, PathBuf)> = Vec::new();
    let roots = std::iter::once(plugin_dir.join("skills")).chain(extra.iter().cloned());
    for root in roots {
        if root != plugin_dir && root.join(SKILL_FILE).is_file() {
            push_skill(&mut out, root);
            continue;
        }
        let Ok(read) = std::fs::read_dir(&root) else {
            continue;
        };
        for path in read.filter_map(Result::ok).map(|e| e.path()) {
            if path.is_dir() && path.join(SKILL_FILE).is_file() {
                push_skill(&mut out, path);
            }
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

fn push_skill(out: &mut Vec<(String, PathBuf)>, dir: PathBuf) {
    if out.iter().any(|(_, seen)| *seen == dir) {
        return;
    }
    if let Some(name) = dir.file_name().and_then(|n| n.to_str()) {
        out.push((name.replace('-', "_"), dir));
    }
}

pub(super) fn import_skill(
    id: &str,
    skill_dir: &Path,
    fallback_category: Option<&str>,
    sink: &Sink,
) -> Result<(), MarketplaceError> {
    let skill_md = skill_dir.join(SKILL_FILE);
    let raw = std::fs::read_to_string(&skill_md)
        .map_err(|e| MarketplaceError::import(&skill_md, "read", e))?;

    let SplitSkillFrontmatter { owned, passthrough } = split_skill_frontmatter(
        authored_skill_frontmatter(&raw)
            .map_err(|e| MarketplaceError::import(&skill_md, "SKILL.md frontmatter", e))?,
    );
    if let Some(passthrough) = &passthrough {
        check_json_compatible(passthrough)
            .map_err(|e| MarketplaceError::import(&skill_md, "SKILL.md", e))?;
    }
    let front: SkillFrontmatter = serde_yaml::from_value(Value::Mapping(owned)).map_err(|e| {
        MarketplaceError::import(&skill_md, "SKILL.md frontmatter is not valid YAML", e)
    })?;

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
            .title
            .or(front.name)
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| id.replace(['_', '-'], " ")),
        description,
        enabled: true,
        file: SKILL_FILE.to_owned(),
        tags: front.tags.map(TagList::into_vec).unwrap_or_default(),
        category: front
            .category
            .or_else(|| fallback_category.map(str::to_owned)),
        display_category: front.display_category,
        hosts: front.hosts,
        frontmatter: passthrough,
    };

    let rel = Path::new("skills").join(id);
    sink.copy_skill_tree(skill_dir, &rel)?;
    sink.write_yaml(&rel.join("config.yaml"), &doc)
}
