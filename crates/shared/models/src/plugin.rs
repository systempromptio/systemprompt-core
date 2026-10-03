//! Plugin component references carried in the signed bridge manifest.
//!
//! A plugin's skill/agent/MCP/content references are [`PluginComponentRef`]s
//! resolved against the instance ([`ComponentSource`]); [`PluginHooksRef`]
//! selects the hooks a plugin materialises and [`PluginDependency`] names a
//! plugin it requires. The services manifest authors them; the gateway and the
//! bridge exchange them at runtime.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ComponentSource {
    Instance,
    #[default]
    Explicit,
}

impl fmt::Display for ComponentSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Instance => write!(f, "instance"),
            Self::Explicit => write!(f, "explicit"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ComponentFilter {
    Enabled,
}

impl fmt::Display for ComponentFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Enabled => write!(f, "enabled"),
        }
    }
}

/// One plugin this plugin requires, in Claude Code's `plugin.json`
/// `dependencies` vocabulary.
///
/// A bare `name` resolves inside the marketplace that carries the dependant;
/// `marketplace` points at another marketplace, which the carrying
/// `MarketplaceConfig` must list in
/// `allow_cross_marketplace_dependencies_on` and, when it is not one of this
/// instance's own marketplaces, declare under `external_marketplaces`.
/// `version` is a semver range Claude Code checks at install time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PluginDependency {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub marketplace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

/// Selects which hooks a plugin materialises into its `hooks/hooks.json`.
///
/// Claude Code executes plugin hooks session-globally — a `PreToolUse` hook
/// with a `*` matcher fires for every tool call regardless of which plugin
/// contributed the tool. One plugin therefore carries the governance hooks for
/// the whole instance; every other plugin emits an empty hooks file.
///
/// `comms` is an opt-in on that same owner: when set, the bridge also installs
/// the `UserPromptSubmit`/`Stop` hooks that drain gateway announcements into
/// the session. Off by default — a tenant that publishes no announcements has
/// no reason to run a command on every prompt.
///
/// `judge` is server-side only: it switches on the conversation judge for the
/// sessions this plugin's track hook reports, and installs no client hook of
/// its own. Like `comms` it rides on the governance owner, so `is_empty`
/// ignores it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginHooksRef {
    #[serde(default)]
    pub governance: bool,
    #[serde(default)]
    pub comms: bool,
    // Why: `judge` was `evaluation` until 5c8d3ae9e. The struct denies unknown
    // fields, so a published kit or a customer config still using the old key
    // is a fatal load failure, not a warning — and kits are pinned OCI
    // artifacts we do not get to edit in lockstep with a core release. The
    // alias keeps them loading; drop it once every pinned kit has been
    // republished on the new name.
    #[serde(default, alias = "evaluation")]
    pub judge: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub include: Vec<String>,
}

impl PluginHooksRef {
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        !self.governance && self.include.is_empty()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginComponentRef {
    #[serde(default)]
    pub source: ComponentSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter: Option<ComponentFilter>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub include: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exclude: Vec<String>,
}
