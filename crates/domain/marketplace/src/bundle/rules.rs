//! Rule component projection: selecting a plugin's rules from the resolved
//! catalogue and laying them out as `rules/<kebab>.md`.
//!
//! Rule ids are canonically `snake_case`; the bundle layout and the markdown
//! `name` use the kebab-case projection Claude Code's rule contract expects.
//! Snake ids contain no hyphens, so the projection is injective — two distinct
//! ids can never collide on a bundle path.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;

use systemprompt_models::bridge::ids::RuleId;
use systemprompt_models::bridge::manifest::RuleEntry;
use systemprompt_models::services::{ComponentSource, PluginConfig};

use super::skills::targets_bundle_hosts;
use super::{BundleContent, BundleFile, PluginBundle};

pub(super) fn append_rule_files(
    config: &PluginConfig,
    content: &BundleContent<'_>,
    bundle: &mut PluginBundle,
) {
    let selected = resolve_rule_ids(config, content);
    for rule in content
        .rules
        .iter()
        .filter(|r| selected.contains(&r.id) && targets_bundle_hosts(&r.hosts))
    {
        let kebab = rule.id.as_str().replace('_', "-");
        bundle.insert(
            format!("rules/{kebab}.md"),
            BundleFile {
                bytes: rule_md(&kebab, rule).into_bytes(),
                executable: false,
            },
        );
    }
}

pub(crate) fn resolve_rule_ids(
    config: &PluginConfig,
    content: &BundleContent<'_>,
) -> BTreeSet<RuleId> {
    let mut ids = BTreeSet::new();
    match config.rules.source {
        ComponentSource::Explicit => {
            for raw in &config.rules.include {
                match RuleId::try_new(raw.as_str()) {
                    Ok(id) => {
                        ids.insert(id);
                    },
                    Err(e) => {
                        tracing::warn!(error = %e, rule_id = raw, "bundle: ignoring invalid rule id");
                    },
                }
            }
        },
        ComponentSource::Instance => {
            for rule in content.rules {
                if !config.rules.exclude.iter().any(|ex| ex == rule.id.as_str()) {
                    ids.insert(rule.id.clone());
                }
            }
        },
    }
    ids
}

fn rule_md(kebab: &str, rule: &RuleEntry) -> String {
    format!(
        "---\nname: {kebab}\ndescription: \"{}\"\n---\n\n{}\n",
        rule.description.replace('"', "\\\""),
        rule.instructions.trim()
    )
}
