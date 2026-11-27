//! Projects and sessions: where the work happens and the conversations held
//! there.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::id::{MessageId, PartId, ProjectId, SessionId};
use crate::model::JsonMap;

/// A worktree the user works in; sessions belong to one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    pub id: ProjectId,
    pub worktree: PathBuf,
    pub name: String,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Lines and files a session changed, shown beside its title.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffSummary {
    pub additions: u64,
    pub deletions: u64,
    pub files: u64,
}

/// Where a session was reverted to: messages from `message_id` on are hidden
/// until the revert is committed (they are deleted) or cleared (they come
/// back).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevertMarker {
    pub message_id: MessageId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part_id: Option<PartId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diff: Option<String>,
}

/// One conversation. A session with a `parent_id` is a subagent run started
/// from its parent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Session {
    pub id: SessionId,
    pub project_id: ProjectId,
    pub parent_id: Option<SessionId>,
    pub directory: PathBuf,
    pub title: String,
    pub agent: Option<String>,
    pub model: Option<String>,
    // JSON: the permission ruleset belongs to the engine's permission layer; the store keeps it
    // verbatim so a ruleset written by a newer engine survives a round trip.
    pub permission: Option<Value>,
    pub revert: Option<RevertMarker>,
    pub summary: DiffSummary,
    pub metadata: JsonMap,
    pub created_at: i64,
    pub updated_at: i64,
    pub archived_at: Option<i64>,
}

impl Session {
    pub const fn is_child(&self) -> bool {
        self.parent_id.is_some()
    }

    pub const fn is_archived(&self) -> bool {
        self.archived_at.is_some()
    }
}

/// The fields a caller chooses when creating a session; the store fills in the
/// rest.
#[derive(Debug, Clone, PartialEq)]
pub struct NewSession {
    pub project_id: ProjectId,
    pub parent_id: Option<SessionId>,
    pub directory: PathBuf,
    pub title: String,
    pub agent: Option<String>,
    pub model: Option<String>,
    // JSON: the permission ruleset belongs to the engine's permission layer and is stored
    // verbatim.
    pub permission: Option<Value>,
    pub metadata: JsonMap,
}

impl NewSession {
    pub fn new(project_id: ProjectId, directory: impl Into<PathBuf>) -> Self {
        Self {
            project_id,
            parent_id: None,
            directory: directory.into(),
            title: String::new(),
            agent: None,
            model: None,
            permission: None,
            metadata: JsonMap::new(),
        }
    }
}
