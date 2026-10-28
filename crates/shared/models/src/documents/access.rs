//! Document access levels, the actions they gate, and the provenance
//! vocabulary recorded on every revision.
//!
//! [`AccessLevel`] is totally ordered: holding a level grants every level
//! below it, so a grant check is a single comparison against
//! [`DocumentAction::required_level`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::str::FromStr;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown {kind} `{value}`")]
pub struct UnknownDocumentValue {
    pub kind: &'static str,
    pub value: String,
}

macro_rules! wire_enum {
    ($name:ident, $kind:literal, { $($variant:ident => $wire:literal),+ $(,)? }) => {
        impl $name {
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $wire),+
                }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl FromStr for $name {
            type Err = UnknownDocumentValue;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    $($wire => Ok(Self::$variant),)+
                    other => Err(UnknownDocumentValue {
                        kind: $kind,
                        value: other.to_owned(),
                    }),
                }
            }
        }
    };
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum AccessLevel {
    View,
    Comment,
    Edit,
    Share,
    Own,
}

wire_enum!(AccessLevel, "access level", {
    View => "view",
    Comment => "comment",
    Edit => "edit",
    Share => "share",
    Own => "own",
});

/// An operation on a document or folder, as named in an authz request.
///
/// `Create` is checked against the destination folder; every other action
/// is checked against the document itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DocumentAction {
    View,
    Comment,
    Edit,
    Share,
    Own,
    Create,
    Delete,
}

wire_enum!(DocumentAction, "document action", {
    View => "view",
    Comment => "comment",
    Edit => "edit",
    Share => "share",
    Own => "own",
    Create => "create",
    Delete => "delete",
});

impl DocumentAction {
    #[must_use]
    pub const fn required_level(self) -> AccessLevel {
        match self {
            Self::View => AccessLevel::View,
            Self::Comment => AccessLevel::Comment,
            Self::Edit | Self::Create => AccessLevel::Edit,
            Self::Share => AccessLevel::Share,
            Self::Own | Self::Delete => AccessLevel::Own,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ActorKind {
    User,
    Agent,
    Bridge,
    Import,
    System,
}

wire_enum!(ActorKind, "document actor kind", {
    User => "user",
    Agent => "agent",
    Bridge => "bridge",
    Import => "import",
    System => "system",
});

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RevisionOriginKind {
    McpExecution,
    BridgeDevice,
    WebSession,
    EditSession,
    Api,
}

wire_enum!(RevisionOriginKind, "revision origin kind", {
    McpExecution => "mcp_execution",
    BridgeDevice => "bridge_device",
    WebSession => "web_session",
    EditSession => "edit_session",
    Api => "api",
});

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProposalStatus {
    Open,
    Accepted,
    Rejected,
    Superseded,
}

wire_enum!(ProposalStatus, "proposal status", {
    Open => "open",
    Accepted => "accepted",
    Rejected => "rejected",
    Superseded => "superseded",
});

/// How an agent's edit under a folder lands: straight onto the head, or as
/// a proposal a reviewer accepts. A folder inherits `propose` from the
/// nearest ancestor that sets it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AgentEditPolicy {
    #[default]
    Direct,
    Propose,
}

wire_enum!(AgentEditPolicy, "agent edit policy", {
    Direct => "direct",
    Propose => "propose",
});

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EditMode {
    Land,
    Propose,
}

wire_enum!(EditMode, "edit mode", {
    Land => "land",
    Propose => "propose",
});
