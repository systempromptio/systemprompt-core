//! Ownership a bundle actually ships, derived from its verified file list.
//!
//! The `owns` block in a manifest is written by the packer and is only a
//! cross-check: verification refuses a manifest whose shipped files reach a
//! directory or id it does not declare, and composition decides every claim
//! from [`shipped_ownership`], never from the declared block.
//!
//! Directories split in two. The marketplace directories
//! ([`MARKETPLACE_BUNDLE_DIRS`]: marketplaces, plugins, skills, rules, hooks,
//! artifacts) are shared: several bundles may contribute to each, because
//! every entry lives under its own id subdirectory and ids are claimed
//! exclusively. Every other allowed directory (`access-control/`, `gateway/`,
//! `config/`, `ai/`, `agents/`, `mcp/`, …) holds instance-wide configuration
//! keyed by file name rather than by id, so only the base bundle (member
//! zero) may ship it; a second contributor could only replace the base's
//! files.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;

use systemprompt_manifest::services::bundle::{
    BUNDLE_ALLOWED_DIRS, BundleOwnership, FileEntry, MARKETPLACE_BUNDLE_DIRS,
};

use super::error::{BundleResult, VerifyFailure};

pub(crate) const OWNED_ID_DIRS: &[&str] = &[
    "marketplaces",
    "plugins",
    "skills",
    "rules",
    "hooks",
    "artifacts",
];

#[must_use]
pub fn is_shared_dir(dir: &str) -> bool {
    MARKETPLACE_BUNDLE_DIRS.contains(&dir)
}

pub fn shipped_ownership(files: &[FileEntry]) -> BundleResult<BundleOwnership> {
    let mut dirs = BTreeSet::new();
    let mut ids: [BTreeSet<String>; 6] = Default::default();
    for file in files {
        let segments = path_segments(&file.path)?;
        let top = segments[0];
        dirs.insert(top.to_owned());
        if segments.len() >= 3
            && let Some(slot) = OWNED_ID_DIRS.iter().position(|d| *d == top)
        {
            ids[slot].insert(segments[1].to_owned());
        }
    }
    let [marketplaces, plugins, skills, rules, hooks, artifacts] = ids.map(sorted);
    Ok(BundleOwnership {
        marketplaces,
        plugins,
        skills,
        rules,
        hooks,
        artifacts,
        dirs: sorted(dirs),
    })
}

pub(crate) fn path_segments(path: &str) -> BundleResult<Vec<&str>> {
    let segments: Vec<&str> = path.split('/').collect();
    let unsafe_segment = segments
        .iter()
        .any(|s| s.is_empty() || *s == "." || *s == ".." || s.contains('\\'));
    if unsafe_segment || segments.len() < 2 || !BUNDLE_ALLOWED_DIRS.contains(&segments[0]) {
        return Err(VerifyFailure::UnsafePath {
            path: path.to_owned(),
        }
        .into());
    }
    Ok(segments)
}

pub(crate) fn require_declared(
    declared: &BundleOwnership,
    shipped: &BundleOwnership,
) -> BundleResult<()> {
    let pairs: [(&str, &Vec<String>, &Vec<String>); 7] = [
        ("directory", &declared.dirs, &shipped.dirs),
        ("marketplace", &declared.marketplaces, &shipped.marketplaces),
        ("plugin", &declared.plugins, &shipped.plugins),
        ("skill", &declared.skills, &shipped.skills),
        ("rule", &declared.rules, &shipped.rules),
        ("hook", &declared.hooks, &shipped.hooks),
        ("artifact", &declared.artifacts, &shipped.artifacts),
    ];
    for (kind, declared, shipped) in pairs {
        if let Some(id) = shipped.iter().find(|id| !declared.contains(id)) {
            return Err(VerifyFailure::UndeclaredOwnership {
                kind: kind.to_owned(),
                id: id.clone(),
            }
            .into());
        }
    }
    Ok(())
}

fn sorted(set: BTreeSet<String>) -> Vec<String> {
    set.into_iter().collect()
}
