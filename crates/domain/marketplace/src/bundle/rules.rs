//! Rule component projection: `rules/<id>.md`, the layout Claude Code reads a
//! plugin's rules from.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;

use systemprompt_models::services::{ComponentSource, PluginConfig};

use super::{BundleContent, BundleFile, PluginBundle};

pub(super) fn append_rule_files(
    config: &PluginConfig,
    content: &BundleContent<'_>,
    bundle: &mut PluginBundle,
) {
    let selected: BTreeSet<&str> = match config.rules.source {
        ComponentSource::Explicit => config.rules.include.iter().map(String::as_str).collect(),
        ComponentSource::Instance => content
            .rules
            .iter()
            .map(|r| r.id.as_str())
            .filter(|id| !config.rules.exclude.iter().any(|ex| ex == id))
            .collect(),
    };

    for rule in content
        .rules
        .iter()
        .filter(|r| selected.contains(r.id.as_str()))
    {
        bundle.insert(
            format!("rules/{}.md", rule.id.as_str()),
            BundleFile {
                bytes: format!("{}\n", rule.content.trim()).into_bytes(),
                executable: false,
            },
        );
    }
}
