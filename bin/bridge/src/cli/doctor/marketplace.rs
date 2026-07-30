//! Doctor check for the `org-provisioned` marketplace the sync emitter mirrors
//! into `~/.claude`.
//!
//! `sync` skips this emitter silently when the Claude Code CLI is absent, which
//! is the failure mode where everything reports healthy but `claude plugin
//! list` is empty. This names it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::config::paths;
use crate::integration::claude_code_cli::{MARKETPLACE, claude_cli_installed, marketplace_dir};

use super::Check;

#[must_use]
pub fn check_marketplace() -> Check {
    let bin = crate::brand::brand().binary_name;

    if !claude_cli_installed() {
        return Check::warn(
            "org marketplace",
            format!(
                "the Claude Code CLI is not installed, so `{bin} sync` skips the {MARKETPLACE} \
                 marketplace — install it (npm i -g @anthropic-ai/claude-code) and re-run sync"
            ),
        );
    }

    let Some(plugins) = paths::claude_cli_plugins_dir() else {
        return Check::fail(
            "org marketplace",
            "no home directory resolvable, so ~/.claude/plugins has no location",
        );
    };

    let manifest = marketplace_dir(&plugins)
        .join(".claude-plugin")
        .join("marketplace.json");
    if !manifest.is_file() {
        return Check::warn(
            "org marketplace",
            format!("{} not present — run `{bin} sync`", manifest.display()),
        );
    }

    let count = std::fs::read(&manifest)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .and_then(|v| v.get("plugins").and_then(|p| p.as_array()).map(Vec::len));

    match count {
        Some(0) | None => Check::warn(
            "org marketplace",
            format!(
                "{} lists no plugins — check the manifest your gateway serves, then re-run \
                 `{bin} sync`",
                manifest.display()
            ),
        ),
        Some(n) => Check::ok(
            "org marketplace",
            format!("{MARKETPLACE}: {n} plugin(s) registered with the Claude Code CLI"),
        ),
    }
}
