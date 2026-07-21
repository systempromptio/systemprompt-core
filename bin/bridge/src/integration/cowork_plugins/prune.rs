//! Reconciles Cowork's materialised copy of the org-provisioned marketplace
//! against the manifest.
//!
//! Cowork installs each org-provisioned plugin into its own tree under
//! `cowork_plugins/marketplaces/<marketplace>/` and never removes one that has
//! disappeared from the bridge's org-plugins root, so an orphan keeps showing
//! up in its plugin picker long after the manifest dropped it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;

use super::emit::{
    CoworkTarget, EmitError, INSTALLED_PLUGINS_FILE, remove_tree, strip_nested_object_key,
};
use super::upsert::clear_enabled;

pub(super) fn prune_orphans(
    target: &CoworkTarget,
    expected: &[&str],
    marketplace: &str,
) -> Result<Vec<String>, EmitError> {
    let mut pruned = Vec::new();
    for name in orphan_names(target, expected, marketplace) {
        remove_tree(
            &target
                .cowork_plugins_dir
                .join("marketplaces")
                .join(marketplace)
                .join(&name),
        )?;
        remove_tree(
            &target
                .cowork_plugins_dir
                .join("cache")
                .join(marketplace)
                .join(&name),
        )?;
        strip_nested_object_key(
            &target.cowork_plugins_dir.join(INSTALLED_PLUGINS_FILE),
            "plugins",
            &format!("{name}@{marketplace}"),
        )?;
        clear_enabled(target, &name, marketplace)?;
        pruned.push(name);
    }

    if !pruned.is_empty() {
        tracing::info!(
            target: "bridge::cowork",
            marketplace,
            pruned = %pruned.join(", "),
            "pruned orphaned plugins from Cowork marketplace"
        );
    }
    Ok(pruned)
}

fn orphan_names(target: &CoworkTarget, expected: &[&str], marketplace: &str) -> Vec<String> {
    let dir = target
        .cowork_plugins_dir
        .join("marketplaces")
        .join(marketplace);
    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|e| e.file_name().to_str().map(str::to_owned))
        .filter(|name| !name.starts_with('.') && !expected.contains(&name.as_str()))
        .collect();
    names.sort();
    names
}
