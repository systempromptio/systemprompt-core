//! Client-facing families a catalog model is advertised to.
//!
//! [`ClientFamily`] names the *client* side of the gateway: which host family
//! lists a model in its picker (Claude Code and Desktop, OpenAI-protocol
//! hosts such as Codex and `OpenCode`, Gemini-protocol hosts). It is
//! independent of the wire the gateway speaks upstream, because the gateway
//! transcodes every inbound wire to every outbound one. A model's
//! `clients: [...]` list is its whole visibility rule, and `clients: []` is
//! "not advertised" — the model still dispatches when asked for by id.
//!
//! [`without_context_variant`] strips the client-side context-window marker
//! from a requested model id, and [`with_context_variant`] adds it, so the
//! bridge profile and the gateway agree on one catalog entry.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum ClientFamily {
    Claude,
    #[serde(rename = "openai")]
    OpenAi,
    Gemini,
}

impl ClientFamily {
    pub const ALL: [Self; 3] = [Self::Claude, Self::OpenAi, Self::Gemini];

    #[must_use]
    pub const fn as_tag(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::OpenAi => "openai",
            Self::Gemini => "gemini",
        }
    }

    #[must_use]
    pub fn from_tag(tag: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|family| family.as_tag() == tag)
    }
}

impl std::fmt::Display for ClientFamily {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_tag())
    }
}

// Why: Claude Code budgets a gateway model at 200k unless the user picks its
// `[1m]` variant (`claude-sonnet-5[1m]`). The suffix is a client-side context
// marker, not a vendor id: it resolves to the base catalog entry and never
// reaches the upstream.
pub const CONTEXT_VARIANT_SUFFIX: &str = "[1m]";

pub const LONG_CONTEXT_WINDOW: u32 = 1_000_000;

#[must_use]
pub fn without_context_variant(requested: &str) -> &str {
    requested
        .strip_suffix(CONTEXT_VARIANT_SUFFIX)
        .unwrap_or(requested)
}

#[must_use]
pub fn has_context_variant(requested: &str) -> bool {
    requested.ends_with(CONTEXT_VARIANT_SUFFIX)
}

#[must_use]
pub fn with_context_variant(model: &str) -> String {
    format!("{}{CONTEXT_VARIANT_SUFFIX}", without_context_variant(model))
}
