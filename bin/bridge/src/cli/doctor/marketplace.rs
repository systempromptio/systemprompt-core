//! Doctor check for the marketplaces the sync emitter mirrors into
//! `~/.claude` — one per marketplace the gateway manifest lists, or the legacy
//! `org-provisioned` one on a gateway that lists none.
//!
//! `sync` skips this emitter silently when the Claude Code CLI is absent, which
//! is the failure mode where everything reports healthy but `claude plugin
//! list` is empty. This names it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use systemprompt_identifiers::MarketplaceId;

use crate::config::paths;
use crate::integration::claude_code_cli::{claude_cli_installed, marketplace_dir, sidecar};

use super::{Check, Status};

const NAME: &str = "org marketplace";

#[must_use]
pub fn check_marketplace() -> Check {
    let bin = crate::brand::brand().binary_name;

    if !claude_cli_installed() {
        return Check::warn(
            NAME,
            format!(
                "the Claude Code CLI is not installed, so `{bin} sync` skips the org marketplaces \
                 — install it (npm i -g @anthropic-ai/claude-code) and re-run sync"
            ),
        );
    }

    let Some(plugins) = paths::claude_cli_plugins_dir() else {
        return Check::fail(
            NAME,
            "no home directory resolvable, so ~/.claude/plugins has no location",
        );
    };

    let owned = match sidecar::owned_marketplaces(&plugins) {
        Ok(owned) => owned,
        Err(e) => return Check::fail(NAME, format!("cannot read the marketplace sidecar: {e}")),
    };
    if owned.is_empty() {
        return Check::warn(
            NAME,
            format!(
                "no marketplace recorded under {} — run `{bin} sync`",
                plugins.display()
            ),
        );
    }
    let mut checks: Vec<Check> = owned
        .iter()
        .map(|marketplace| check_one(&plugins, marketplace, bin))
        .collect();
    checks.extend(owned.iter().filter_map(|m| check_node_tooling(&plugins, m)));
    combine(checks)
}

// Why: a plugin that ships a lockfile loads without its packages when the
// installer is missing, and sync only records that as a warning.
fn check_node_tooling(plugins: &Path, marketplace: &MarketplaceId) -> Option<Check> {
    use crate::sysproc::binary_on_path;
    use systemprompt_models::bridge::plugin_bundle::{NODE_PACKAGE_FILE, node_lockfile};
    let root = marketplace_dir(plugins, marketplace).join("plugins");
    let mut missing = Vec::new();
    for entry in std::fs::read_dir(root).ok()?.flatten() {
        let dir = entry.path();
        if !dir.join(NODE_PACKAGE_FILE).is_file() {
            continue;
        }
        let Some(lockfile) = node_lockfile(&dir) else {
            continue;
        };
        let tool = if lockfile.starts_with("bun.") {
            "bun"
        } else {
            "npm"
        };
        if binary_on_path(tool).is_none() {
            missing.push(format!(
                "{} needs {tool} for its {lockfile}",
                entry.file_name().to_string_lossy()
            ));
        }
    }
    if missing.is_empty() {
        return None;
    }
    Some(Check::warn(
        NAME,
        format!(
            "{marketplace}: Node packages cannot be installed — {} — install the tool and re-run \
             sync",
            missing.join(", ")
        ),
    ))
}

fn combine(mut checks: Vec<Check>) -> Check {
    if checks.len() == 1 {
        return checks.remove(0);
    }
    let status = checks
        .iter()
        .map(|c| c.status)
        .max_by_key(|s| match s {
            Status::Ok => 0,
            Status::Warn => 1,
            Status::Fail => 2,
        })
        .unwrap_or(Status::Warn);
    let detail = checks
        .iter()
        .map(|c| c.detail.as_str())
        .collect::<Vec<_>>()
        .join("; ");
    match status {
        Status::Ok => Check::ok(NAME, detail),
        Status::Warn => Check::warn(NAME, detail),
        Status::Fail => Check::fail(NAME, detail),
    }
}

fn check_one(plugins: &Path, marketplace: &MarketplaceId, bin: &str) -> Check {
    let manifest = marketplace_dir(plugins, marketplace)
        .join(".claude-plugin")
        .join("marketplace.json");
    if !manifest.is_file() {
        return Check::warn(
            NAME,
            format!("{} not present — run `{bin} sync`", manifest.display()),
        );
    }

    let parsed = std::fs::read(&manifest)
        .map_err(|e| e.to_string())
        .and_then(|bytes| {
            serde_json::from_slice::<serde_json::Value>(&bytes).map_err(|e| e.to_string())
        });
    let doc = match parsed {
        Ok(doc) => doc,
        Err(e) => {
            return Check::fail(
                NAME,
                format!(
                    "{} is unreadable or not valid JSON ({e}) — re-run `{bin} sync`",
                    manifest.display()
                ),
            );
        },
    };

    let count = doc.get("plugins").and_then(|p| p.as_array()).map(Vec::len);
    match count {
        Some(0) | None => Check::warn(
            NAME,
            format!(
                "{} lists no plugins — check the manifest your gateway serves, then re-run \
                 `{bin} sync`",
                manifest.display()
            ),
        ),
        Some(n) => Check::ok(
            NAME,
            format!("{marketplace}: {n} plugin(s) registered with the Claude Code CLI"),
        ),
    }
}
