//! Everything the importer tolerates by default and refuses under `strict`.
//!
//! A warning marks a fact the Anthropic tree states that systemprompt cannot
//! represent, or a default the importer had to invent. Under
//! [`ImportOptions::strict`](super::ImportOptions) every variant in
//! [`ImportWarning::is_strict_error`] becomes the error the report would
//! otherwise carry, so a publishing pipeline can refuse a tree that only
//! half-translates.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportWarning {
    NoMarketplaceManifest,
    InlineMcpServers { plugin: String },
    CommandsDirectory { plugin: String },
    AgentsDirectory { plugin: String, count: usize },
    MissingCategory { plugin: String, applied: String },
    UnsupportedHookAction { plugin: String, event: String },
    RemotePluginSource { plugin: String },
    NoSkills { plugin: String },
    UnattachedRootRules { rules: Vec<String> },
}

impl ImportWarning {
    #[must_use]
    pub const fn is_strict_error(&self) -> bool {
        matches!(
            self,
            Self::NoMarketplaceManifest
                | Self::InlineMcpServers { .. }
                | Self::CommandsDirectory { .. }
                | Self::MissingCategory { .. }
                | Self::RemotePluginSource { .. }
                | Self::UnattachedRootRules { .. }
        )
    }
}

impl fmt::Display for ImportWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoMarketplaceManifest => write!(
                f,
                ".claude-plugin/marketplace.json is missing; only the base tree was imported"
            ),
            Self::InlineMcpServers { plugin } => write!(
                f,
                "plugin '{plugin}' defines MCP servers inline; systemprompt MCP servers are \
                 platform-defined and referenced by id from the base bundle"
            ),
            Self::CommandsDirectory { plugin } => write!(
                f,
                "plugin '{plugin}' ships a commands/ directory, which has no systemprompt \
                 equivalent and was not imported"
            ),
            Self::AgentsDirectory { plugin, count } => write!(
                f,
                "plugin '{plugin}' ships {count} agent file(s); agents are platform-defined and \
                 referenced by id, so they were not imported"
            ),
            Self::MissingCategory { plugin, applied } => write!(
                f,
                "plugin '{plugin}' declares no category in its sidecar or marketplace entry; \
                 '{applied}' was applied"
            ),
            Self::UnsupportedHookAction { plugin, event } => write!(
                f,
                "plugin '{plugin}' binds a non-command action to {event}; only command hooks are \
                 importable"
            ),
            Self::RemotePluginSource { plugin } => write!(
                f,
                "plugin '{plugin}' names a non-local source; only a relative path inside the tree \
                 can be imported"
            ),
            Self::NoSkills { plugin } => {
                write!(f, "plugin '{plugin}' ships no skills")
            },
            Self::UnattachedRootRules { rules } => write!(
                f,
                "repository-root rules ({}) belong to no plugin; a rule only reaches a host \
                 through a plugin that lists it, so move them under a plugin's rules/ directory",
                rules.join(", ")
            ),
        }
    }
}
