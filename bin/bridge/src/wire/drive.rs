//! The Drive status as the webview and the tray read it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Serialize;
use systemprompt_identifiers::{DocumentId, UserId};

use crate::drive::{ConflictNotice, DriveStatus, EditorPresence};
use crate::verdict::{Tone, Verdict};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub enum DriveCode {
    Off,
    Synced,
    Pending,
    Conflicts,
    Error,
}

impl DriveCode {
    #[must_use]
    pub const fn of(status: &DriveStatus) -> Self {
        if !status.enabled {
            Self::Off
        } else if status.last_error.is_some() {
            Self::Error
        } else if !status.conflicts.is_empty() {
            Self::Conflicts
        } else if status.pending_uploads > 0 || status.synced_at.is_none() {
            Self::Pending
        } else {
            Self::Synced
        }
    }

    #[must_use]
    pub const fn verdict(self) -> Verdict<Self> {
        let tone = match self {
            Self::Off => Tone::Unknown,
            Self::Synced => Tone::Ok,
            Self::Pending => Tone::Probing,
            Self::Conflicts => Tone::Warn,
            Self::Error => Tone::Err,
        };
        Verdict::new(tone, self)
    }
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub struct DriveConflictPayload {
    pub path: String,
    pub sibling: String,
    pub head_revision_id: String,
    pub detected_at_unix: i64,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub struct DriveEditorPayload {
    #[cfg_attr(feature = "ts-export", ts(type = "string"))]
    pub document_id: DocumentId,
    pub path: Option<String>,
    #[cfg_attr(feature = "ts-export", ts(type = "string"))]
    pub user_id: UserId,
    pub editing: bool,
    pub seen_at_unix: i64,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub struct DriveStatusPayload {
    pub verdict: Verdict<DriveCode>,
    pub root: Option<String>,
    pub synced_at_unix: Option<i64>,
    pub pending_uploads: u32,
    pub editing_count: u32,
    pub conflicts: Vec<DriveConflictPayload>,
    pub editors: Vec<DriveEditorPayload>,
    pub last_error: Option<String>,
}

impl From<&DriveStatus> for DriveStatusPayload {
    fn from(status: &DriveStatus) -> Self {
        Self {
            verdict: DriveCode::of(status).verdict(),
            root: status.root.as_ref().map(|p| p.display().to_string()),
            synced_at_unix: status.synced_at.map(|t| t.timestamp()),
            pending_uploads: u32::try_from(status.pending_uploads).unwrap_or(u32::MAX),
            editing_count: u32::try_from(status.editing_count()).unwrap_or(u32::MAX),
            conflicts: status.conflicts.iter().map(conflict).collect(),
            editors: status.editors.iter().map(editor).collect(),
            last_error: status.last_error.clone(),
        }
    }
}

fn conflict(notice: &ConflictNotice) -> DriveConflictPayload {
    DriveConflictPayload {
        path: notice.path.clone(),
        sibling: notice.sibling.clone(),
        head_revision_id: notice.head_revision_id.as_str().to_owned(),
        detected_at_unix: notice.detected_at.timestamp(),
    }
}

fn editor(presence: &EditorPresence) -> DriveEditorPayload {
    DriveEditorPayload {
        document_id: presence.document_id.clone(),
        path: presence.path.clone(),
        user_id: presence.user_id.clone(),
        editing: presence.state == systemprompt_models::documents::PresenceState::Editing,
        seen_at_unix: presence.seen_at.timestamp(),
    }
}
