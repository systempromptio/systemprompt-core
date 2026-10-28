//! Documents shared with this user since the Drive folder last caught up.
//!
//! A `document.shared` notice is queued on the handle, recorded into the
//! state by the next cycle, and dropped once that document is on disk, so
//! `drive status` lists the shares the folder has not synced yet.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{DocumentId, UserId};

use super::state::DriveState;
use crate::proxy::drive_signal::{DocumentNotice, NoticeKind};

/// One share received by this user, as much of it as the frame carried.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReceivedShare {
    pub document_id: DocumentId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shared_by: Option<UserId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<String>,
    pub received_at: DateTime<Utc>,
}

#[must_use]
pub fn received(notice: &DocumentNotice, at: DateTime<Utc>) -> Option<ReceivedShare> {
    (notice.kind == NoticeKind::Shared).then(|| ReceivedShare {
        document_id: notice.document_id.clone(),
        document_name: notice.document_name.clone(),
        shared_by: Some(notice.from.clone()),
        level: notice.detail.clone(),
        received_at: at,
    })
}

pub fn record(state: &mut DriveState, shares: Vec<ReceivedShare>) -> bool {
    let changed = !shares.is_empty();
    for share in shares {
        state.shares.retain(|s| s.document_id != share.document_id);
        state.shares.push(share);
    }
    changed
}

pub fn prune_synced(state: &mut DriveState) -> bool {
    let before = state.shares.len();
    let synced: Vec<DocumentId> = state
        .shares
        .iter()
        .filter(|s| state.path_of(&s.document_id).is_some())
        .map(|s| s.document_id.clone())
        .collect();
    state.shares.retain(|s| !synced.contains(&s.document_id));
    before != state.shares.len()
}
