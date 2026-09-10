//! `rules/*.md` → `rules/<name>/config.yaml` plus the markdown body.
//!
//! Rules may sit at the repo root or inside a plugin; both flatten into the one
//! `rules/` directory the bundle format defines, so a name claimed twice is an
//! error rather than a silent overwrite.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;
use std::path::Path;

use serde::Deserialize;
use systemprompt_identifiers::PluginRuleId;
use systemprompt_models::services::frontmatter::split_frontmatter;
use systemprompt_models::services::{DEFAULT_RULE_CONTENT_FILE, DiskRuleConfig};

use crate::error::MarketplaceError;

use super::writer::Sink;

#[derive(Debug, Clone, Default, Deserialize)]
struct RuleFrontmatter {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    description: Option<String>,
}

pub(super) fn import_rules_dir(
    rules_dir: &Path,
    seen: &mut BTreeSet<String>,
    sink: &Sink,
    imported: &mut Vec<String>,
) -> Result<(), MarketplaceError> {
    if !rules_dir.is_dir() {
        return Ok(());
    }

    let read = std::fs::read_dir(rules_dir).map_err(|e| MarketplaceError::Import {
        path: rules_dir.display().to_string(),
        message: e.to_string(),
    })?;
    let mut files: Vec<std::path::PathBuf> = read
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|x| x == "md"))
        .collect();
    files.sort();

    for path in files {
        let Some(name) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if !seen.insert(name.to_owned()) {
            return Err(MarketplaceError::Import {
                path: path.display().to_string(),
                message: format!("rule '{name}' is defined more than once in the tree"),
            });
        }
        import_rule(name, &path, sink)?;
        imported.push(name.to_owned());
    }

    Ok(())
}

fn import_rule(name: &str, path: &Path, sink: &Sink) -> Result<(), MarketplaceError> {
    let raw = std::fs::read_to_string(path).map_err(|e| MarketplaceError::Import {
        path: path.display().to_string(),
        message: e.to_string(),
    })?;

    let front: RuleFrontmatter = match split_frontmatter(&raw) {
        Some(f) => serde_yaml::from_str(f.yaml).map_err(|e| MarketplaceError::Import {
            path: path.display().to_string(),
            message: format!("rule frontmatter is not valid YAML: {e}"),
        })?,
        None => RuleFrontmatter::default(),
    };

    let doc = DiskRuleConfig {
        id: PluginRuleId::new(name),
        name: front
            .name
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| name.replace(['_', '-'], " ")),
        description: front.description.unwrap_or_default(),
        enabled: true,
        file: DEFAULT_RULE_CONTENT_FILE.to_owned(),
    };

    let rel = Path::new("rules").join(name);
    sink.copy_file(path, &rel.join(DEFAULT_RULE_CONTENT_FILE))?;
    sink.write_yaml(&rel.join("config.yaml"), &doc)
}
