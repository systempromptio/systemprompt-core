//! [`AgentCap`], the most an agent may do on a document, and
//! [`AgentCapability`], the actions an agent may never take unless the
//! policy allows them.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{DocumentFolderId, DocumentId};
use systemprompt_models::documents::{AccessLevel, DocumentAction, UnknownDocumentValue};

/// Ordered from the most to the least restrictive, so `min` of two caps is
/// the stricter one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentCap {
    None,
    Read,
    Comment,
    Suggest,
    Edit,
}

impl AgentCap {
    pub const ALL: [Self; 5] = [
        Self::None,
        Self::Read,
        Self::Comment,
        Self::Suggest,
        Self::Edit,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Read => "read",
            Self::Comment => "comment",
            Self::Suggest => "suggest",
            Self::Edit => "edit",
        }
    }

    /// The highest level an agent holds under this cap; `None` hides the
    /// document. `edit` leaves the user's level as it is, since the actions
    /// above `edit` are gated by [`AgentCapability`] instead.
    #[must_use]
    pub fn cap_level(self, held: AccessLevel) -> Option<AccessLevel> {
        let ceiling = match self {
            Self::None => return None,
            Self::Edit => return Some(held),
            Self::Read => AccessLevel::View,
            Self::Comment | Self::Suggest => AccessLevel::Comment,
        };
        Some(held.min(ceiling))
    }
}

impl fmt::Display for AgentCap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for AgentCap {
    type Err = UnknownDocumentValue;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|cap| cap.as_str() == value)
            .ok_or_else(|| UnknownDocumentValue {
                kind: "agent cap",
                value: value.to_owned(),
            })
    }
}

/// Actions an agent may not take on any cap unless the registered policy
/// allows them for that document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentCapability {
    /// Share, change or revoke grants, and answer access requests.
    Share,
    /// Grant `comment` to a person by mentioning them.
    MentionGrant,
    /// Trash, restore from trash, or delete.
    Trash,
    /// Move, transfer ownership, revoke every grant: the owner's actions.
    Administer,
    /// Restore an earlier revision over the head.
    RestoreRevision,
    /// Replace a document's whole content in one write.
    ReplaceContent,
    /// Change a document template.
    EditTemplate,
}

impl AgentCapability {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Share => "share",
            Self::MentionGrant => "mention_grant",
            Self::Trash => "trash",
            Self::Administer => "administer",
            Self::RestoreRevision => "restore_revision",
            Self::ReplaceContent => "replace_content",
            Self::EditTemplate => "edit_template",
        }
    }

    /// The capability an access check for `action` exercises, if any.
    #[must_use]
    pub const fn of_action(action: DocumentAction) -> Option<Self> {
        match action {
            DocumentAction::Share => Some(Self::Share),
            DocumentAction::Delete => Some(Self::Trash),
            DocumentAction::Own => Some(Self::Administer),
            DocumentAction::View
            | DocumentAction::Comment
            | DocumentAction::Edit
            | DocumentAction::Create => None,
        }
    }
}

impl fmt::Display for AgentCapability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Which source set the effective cap.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CapSource {
    Policy { policy: &'static str },
    DocumentOverride { document_id: DocumentId },
    FolderOverride { folder_id: DocumentFolderId },
}

impl CapSource {
    /// The short form the read audit stores: `policy:<name>`,
    /// `document:<id>` or `folder:<id>`.
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::Policy { policy } => format!("policy:{policy}"),
            Self::DocumentOverride { document_id } => format!("document:{document_id}"),
            Self::FolderOverride { folder_id } => format!("folder:{folder_id}"),
        }
    }
}

/// The cap an agent acts under on one target, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EffectiveCap {
    pub cap: AgentCap,
    pub policy_cap: AgentCap,
    pub source: CapSource,
}

impl EffectiveCap {
    #[must_use]
    pub fn reason(&self) -> String {
        match &self.source {
            CapSource::Policy { policy } => {
                format!("the {policy} agent policy caps agents at {}", self.cap)
            },
            CapSource::DocumentOverride { .. } => format!(
                "the document's owner capped agents at {} (policy allows {})",
                self.cap, self.policy_cap
            ),
            CapSource::FolderOverride { folder_id } => format!(
                "folder {folder_id} caps agents at {} (policy allows {})",
                self.cap, self.policy_cap
            ),
        }
    }
}
