//! Editing-presence beacons for documents this device is changing.
//!
//! A beacon is raised when a watched file changes or an agent hook reports
//! touching one; the bridge never reports `Viewing`. Beacons are throttled to
//! one per document per [`PRESENCE_WINDOW`], since an editor saving every
//! keystroke would otherwise beacon at the rate it writes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::path::Path;
use std::time::{Duration, Instant};

use systemprompt_identifiers::DocumentId;
use systemprompt_models::documents::{PresenceBeacon, PresenceSource, PresenceState};

use super::Cycle;
use super::error::DriveResult;
use super::root::DriveRoot;
use super::state::DriveState;

pub const PRESENCE_WINDOW: Duration = Duration::from_secs(10);

#[derive(Debug, Default)]
pub struct PresenceThrottle {
    last: HashMap<DocumentId, Instant>,
}

impl PresenceThrottle {
    pub fn admit(&mut self, document_id: &DocumentId, now: Instant) -> bool {
        match self.last.get(document_id) {
            Some(sent) if now.saturating_duration_since(*sent) < PRESENCE_WINDOW => false,
            _ => {
                self.last.insert(document_id.clone(), now);
                true
            },
        }
    }
}

/// The document a touched path belongs to, when the touch is a person's edit.
/// The bridge's own pull writes the synced revision's bytes and the watcher
/// reports that write like any other, so a file whose bytes still hash to
/// the synced revision was not edited and files no beacon.
#[must_use]
pub fn document_for(root: &DriveRoot, state: &DriveState, path: &Path) -> Option<DocumentId> {
    let rel = root.relative(path)?;
    let entry = state.entries.get(&rel)?;
    let unchanged = super::hash::hash_file(path).is_ok_and(|(sha, _)| sha == entry.sha256);
    if unchanged {
        return None;
    }
    entry.document_id.clone()
}

#[must_use]
pub fn editing_beacon(document_id: DocumentId, source_ref: &str) -> PresenceBeacon {
    PresenceBeacon {
        document_id,
        state: PresenceState::Editing,
        source_kind: PresenceSource::Bridge,
        source_ref: source_ref.to_owned(),
        cursor: None,
    }
}

pub async fn send_editing(
    cycle: Cycle<'_>,
    throttle: &mut PresenceThrottle,
    documents: Vec<DocumentId>,
    now: Instant,
) -> DriveResult<usize> {
    let mut sent = 0;
    for document_id in documents {
        if !throttle.admit(&document_id, now) {
            continue;
        }
        let beacon = editing_beacon(document_id, cycle.device_id.as_str());
        cycle.client.presence(cycle.bearer, &beacon).await?;
        sent += 1;
    }
    Ok(sent)
}
