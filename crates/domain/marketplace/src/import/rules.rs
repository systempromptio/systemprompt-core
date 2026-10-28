//! `rules/*.md` → `rules/<name>/config.yaml` plus the markdown body.
//!
//! Rules may sit at the repo root or inside a plugin; both flatten into the one
//! `rules/` directory the bundle format defines, so a name claimed twice is an
//! error rather than a silent overwrite. Rule ids are canonically `snake_case`
//! and the bundle lays them out as `rules/<kebab>.md`, so an imported file stem
//! is normalised by replacing hyphens with underscores; `my-rule.md` and
//! `my_rule.md` name the same rule and the duplicate check sees both.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;
use std::path::Path;

use serde::Deserialize;
use systemprompt_identifiers::MarketplaceRuleId;
use systemprompt_manifest::services::frontmatter::split_frontmatter;
use systemprompt_manifest::services::{DEFAULT_RULE_CONTENT_FILE, DiskRuleConfig};
use systemprompt_models::bridge::host::{HostKind, HostProduct};

use crate::error::MarketplaceError;

use super::writer::Sink;

#[derive(Debug, Clone, Default, Deserialize)]
struct RuleFrontmatter {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    hosts: Vec<String>,
}

pub(super) fn import_rules_dir(
    rules_dir: &Path,
    seen: &mut BTreeSet<String>,
    sink: &Sink,
    imported: &mut Vec<String>,
) -> Result<Vec<String>, MarketplaceError> {
    let mut without_opencode = Vec::new();
    if !rules_dir.is_dir() {
        return Ok(without_opencode);
    }

    let read =
        std::fs::read_dir(rules_dir).map_err(|e| MarketplaceError::import(rules_dir, "read", e))?;
    let mut files: Vec<std::path::PathBuf> = read
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|x| x == "md"))
        .collect();
    files.sort();

    for path in files {
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let name = stem.replace('-', "_");
        if !seen.insert(name.clone()) {
            return Err(MarketplaceError::Import {
                path: path.display().to_string(),
                message: format!("rule '{name}' is defined more than once in the tree"),
            });
        }
        if !import_rule(&name, &path, sink)? {
            without_opencode.push(name.clone());
        }
        imported.push(name);
    }

    Ok(without_opencode)
}

fn import_rule(name: &str, path: &Path, sink: &Sink) -> Result<bool, MarketplaceError> {
    let raw =
        std::fs::read_to_string(path).map_err(|e| MarketplaceError::import(path, "read", e))?;

    let front: RuleFrontmatter = match split_frontmatter(&raw) {
        Some(f) => serde_yaml::from_str(f.yaml)
            .map_err(|e| MarketplaceError::import(path, "rule frontmatter is not valid YAML", e))?,
        None => RuleFrontmatter::default(),
    };

    if let Err(unknown) = HostKind::validate_targets(&front.hosts) {
        return Err(MarketplaceError::import(
            path,
            "rule frontmatter `hosts`",
            unknown,
        ));
    }

    let id = MarketplaceRuleId::try_new(name)
        .map_err(|e| MarketplaceError::import(path, "rule id", e))?;
    let doc = DiskRuleConfig {
        id,
        name: front
            .name
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| name.replace('_', " ")),
        description: front.description.unwrap_or_default(),
        enabled: true,
        file: DEFAULT_RULE_CONTENT_FILE.to_owned(),
        tags: Vec::new(),
        hosts: front.hosts,
    };

    let rel = Path::new("rules").join(name);
    sink.copy_file(path, &rel.join(DEFAULT_RULE_CONTENT_FILE))?;
    sink.write_yaml(&rel.join("config.yaml"), &doc)?;
    Ok(HostProduct::OpenCode.targeted_by(&doc.hosts))
}
