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

    let owned = match sidecar::owned_marketplaces(&plugins, sidecar::Legacy::WhenUnrecorded) {
        Ok(owned) => owned,
        Err(e) => return Check::fail(NAME, format!("cannot read the marketplace sidecar: {e}")),
    };
    let checks: Vec<Check> = owned
        .iter()
        .map(|marketplace| check_one(&plugins, marketplace, bin))
        .collect();
    combine(checks)
}

// Why: one line per marketplace would bury a single broken one among healthy
// siblings, so the worst status wins and every detail is kept.
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
