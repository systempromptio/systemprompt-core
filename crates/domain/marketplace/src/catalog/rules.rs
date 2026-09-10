//! Projects on-disk rule directories into the signed `RuleEntry` records the
//! manifest carries.
//!
//! A rule whose descriptor disagrees with its directory, or whose content file
//! is missing, fails the load rather than being skipped: a signed entry with
//! empty instructions would be a silent fail-open. Rule text is trimmed before
//! it is hashed, so a file that differs only by a trailing newline does not
//! change the bundle content version.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use sha2::{Digest, Sha256};
use systemprompt_models::bridge::ids::{RuleName, Sha256Digest};
use systemprompt_models::bridge::manifest::RuleEntry;
use systemprompt_models::services::{DiskRuleConfig, RULE_CONFIG_FILENAME, strip_frontmatter};

use crate::error::MarketplaceError;
use crate::trace::{NoopTrace, TraceEvent, TraceKind, TraceSink, TraceStage};

pub fn load_rules(services_root: &Path) -> Result<Vec<RuleEntry>, MarketplaceError> {
    load_rules_traced(services_root, &mut NoopTrace)
}

pub fn load_rules_traced(
    services_root: &Path,
    trace: &mut dyn TraceSink,
) -> Result<Vec<RuleEntry>, MarketplaceError> {
    let rules_dir = services_root.join("rules");
    if !rules_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut entries: Vec<(String, std::path::PathBuf)> = Vec::new();
    let read =
        std::fs::read_dir(&rules_dir).map_err(|e| MarketplaceError::Catalog(e.to_string()))?;
    for entry in read {
        let entry = entry.map_err(|e| MarketplaceError::Catalog(e.to_string()))?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(dir_name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !path.join(RULE_CONFIG_FILENAME).exists() {
            tracing::warn!(
                rule_dir = %path.display(),
                "manifest: rule directory has no config.yaml; skipping"
            );
            trace.record(TraceEvent {
                kind: TraceKind::Rule,
                id: dir_name.to_owned(),
                stage: TraceStage::DiskScan,
                reason: "rule directory has no config.yaml".to_owned(),
            });
            continue;
        }
        entries.push((dir_name.to_owned(), path));
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let mut out = Vec::with_capacity(entries.len());
    for (dir_name, rule_dir) in entries {
        match build_rule_entry(&dir_name, &rule_dir) {
            Ok(Some(entry)) => out.push(entry),
            Ok(None) => {
                tracing::info!(
                    rule_dir = %rule_dir.display(),
                    "manifest: rule is disabled; skipping"
                );
                trace.record(TraceEvent {
                    kind: TraceKind::Rule,
                    id: dir_name,
                    stage: TraceStage::Disabled,
                    reason: "rule config sets enabled: false".to_owned(),
                });
            },
            Err(e) => {
                tracing::error!(
                    rule_dir = %rule_dir.display(),
                    error = %e,
                    "manifest: failed to build rule entry"
                );
                trace.record(TraceEvent {
                    kind: TraceKind::Rule,
                    id: dir_name,
                    stage: TraceStage::Parse,
                    reason: e.to_string(),
                });
                return Err(e);
            },
        }
    }
    Ok(out)
}

fn build_rule_entry(
    dir_name: &str,
    rule_dir: &Path,
) -> Result<Option<RuleEntry>, MarketplaceError> {
    let config_path = rule_dir.join(RULE_CONFIG_FILENAME);
    let config_text = std::fs::read_to_string(&config_path)
        .map_err(|e| MarketplaceError::Catalog(e.to_string()))?;
    let config: DiskRuleConfig = serde_yaml::from_str(&config_text)
        .map_err(|e| MarketplaceError::Catalog(format!("parse {}: {e}", config_path.display())))?;

    if !config.enabled {
        return Ok(None);
    }

    if config.id.as_str() != dir_name {
        return Err(MarketplaceError::Catalog(format!(
            "rule id '{}' does not match its directory name '{dir_name}'",
            config.id.as_str()
        )));
    }
    let display_name = if config.name.is_empty() {
        dir_name.replace('_', " ")
    } else {
        config.name.clone()
    };
    let name =
        RuleName::try_new(display_name).map_err(|e| MarketplaceError::Catalog(e.to_string()))?;

    let content_path = rule_dir.join(config.content_file());
    if !content_path.exists() {
        return Err(MarketplaceError::Catalog(format!(
            "rule '{dir_name}' names content file '{}' which does not exist",
            config.content_file()
        )));
    }
    let raw = std::fs::read_to_string(&content_path)
        .map_err(|e| MarketplaceError::Catalog(e.to_string()))?;
    let instructions = strip_frontmatter(&raw).trim().to_owned();

    let mut hasher = Sha256::new();
    hasher.update(instructions.as_bytes());
    let sha256 = Sha256Digest::try_new(hex::encode(hasher.finalize()))
        .map_err(|e| MarketplaceError::Catalog(e.to_string()))?;

    Ok(Some(RuleEntry {
        id: config.id,
        name,
        description: config.description,
        file_path: content_path.to_string_lossy().into_owned(),
        tags: config.tags,
        sha256,
        instructions,
        hosts: config.hosts,
        plugins: Vec::new(),
    }))
}
