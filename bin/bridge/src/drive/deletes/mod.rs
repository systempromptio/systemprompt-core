//! A file the user removed from the Drive folder, carried to the server.
//!
//! A cycle first [`observe`]s which synced files are absent, before the pull,
//! so the pull neither restores them nor mistakes them for gone. After the
//! pull, [`apply`] acts on them:
//!
//! - A new local file with the same digest as exactly one absent file is that
//!   file renamed, so the document moves and keeps its history. A move the
//!   server refuses puts the file back under its old name.
//! - An absent file is trashed only once it has been absent for two cycles and
//!   [`CONFIRM_SECS`], because editors save by deleting and re-creating.
//! - A missing or empty root, or more absences than a person plausibly meant
//!   ([`is_bulk`]), trashes nothing: the deletes are held until the user
//!   chooses with `drive deletes apply` or `drive deletes restore`.
//! - The trash request names the synced revision. A newer head, or a refusal,
//!   restores the head into the folder with a notice.
//!
//! A trashed document stays restorable from the Drive trash.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod moves;
mod trash;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use self::moves::{KnownIds, apply_moves};
use self::trash::{restore_all, trash_one};
use super::Cycle;
use super::error::{DriveError, DriveResult};
use super::root::DriveRoot;
use super::scan::walk;
use super::state::DriveState;

pub const CONFIRM_CYCLES: u32 = 2;
pub const CONFIRM_SECS: i64 = 15;
pub const BULK_ABSOLUTE: usize = 10;
pub const BULK_SMALL_FOLDER: usize = 3;
const NOTICES_KEPT: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingDelete {
    pub noticed_at: DateTime<Utc>,
    pub cycles: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HeldReason {
    RootEmpty,
    Bulk,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeldDeletes {
    pub reason: HeldReason,
    pub count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HeldChoice {
    Apply,
    Restore,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeleteNotice {
    pub path: String,
    pub reason: String,
    pub at: DateTime<Utc>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DeleteReport {
    pub trashed: usize,
    pub moved: usize,
    pub restored: usize,
    pub held: Option<HeldDeletes>,
}

/// Records which synced files are absent. A file that came back, or an entry
/// the feed already removed, stops being pending.
pub fn observe(root: &DriveRoot, state: &mut DriveState, now: DateTime<Utc>) -> bool {
    let before = state.pending_deletes.clone();
    let absent: Vec<String> = state
        .entries
        .keys()
        .filter(|rel| root.file(rel).is_ok_and(|path| !path.exists()))
        .cloned()
        .collect();
    state.pending_deletes.retain(|rel, _| absent.contains(rel));
    for rel in absent {
        state
            .pending_deletes
            .entry(rel)
            .and_modify(|pending| pending.cycles += 1)
            .or_insert(PendingDelete {
                noticed_at: now,
                cycles: 1,
            });
    }
    if state.pending_deletes.is_empty() {
        state.held_deletes = None;
        state.held_choice = None;
    }
    before != state.pending_deletes
}

#[must_use]
pub const fn is_bulk(absent: usize, synced: usize) -> bool {
    absent > BULK_ABSOLUTE || (absent > BULK_SMALL_FOLDER && absent * 2 > synced)
}

#[must_use]
pub fn is_confirmed(pending: &PendingDelete, now: DateTime<Utc>) -> bool {
    pending.cycles >= CONFIRM_CYCLES && now - pending.noticed_at >= Duration::seconds(CONFIRM_SECS)
}

fn root_looks_lost(root: &DriveRoot, state: &DriveState) -> DriveResult<bool> {
    if !root.dir().is_dir() {
        return Ok(true);
    }
    Ok(state.entries.len() > 1 && walk(root)?.is_empty())
}

pub async fn apply(cycle: Cycle<'_>, state: &mut DriveState) -> DriveResult<DeleteReport> {
    let mut report = DeleteReport::default();
    if state.pending_deletes.is_empty() {
        return Ok(report);
    }
    let lost = root_looks_lost(cycle.root, state)?;
    let mut known_ids = KnownIds::default();
    if !lost {
        apply_moves(cycle, state, &mut known_ids, &mut report).await?;
    }
    let absent = state.pending_deletes.len();
    if absent == 0 {
        state.save(&cycle.root.state_path())?;
        return Ok(report);
    }
    let choice = state.held_choice.take();
    if choice == Some(HeldChoice::Restore) {
        restore_all(cycle, state, &mut report).await?;
        state.held_deletes = None;
        state.save(&cycle.root.state_path())?;
        return Ok(report);
    }
    let reason = if lost {
        Some(HeldReason::RootEmpty)
    } else if is_bulk(absent, state.entries.len()) {
        Some(HeldReason::Bulk)
    } else {
        None
    };
    if let (Some(reason), None) = (reason, choice) {
        let held = HeldDeletes {
            reason,
            count: absent,
        };
        tracing::warn!(?reason, count = absent, "drive is holding local deletes");
        state.held_deletes = Some(held);
        report.held = Some(held);
        state.save(&cycle.root.state_path())?;
        return Ok(report);
    }
    state.held_deletes = None;
    let now = Utc::now();
    let due: Vec<String> = state
        .pending_deletes
        .iter()
        .filter(|(_, pending)| choice.is_some() || is_confirmed(pending, now))
        .map(|(rel, _)| rel.clone())
        .collect();
    for rel in due {
        trash_one(cycle, state, &rel, &mut known_ids, &mut report).await?;
        state.save(&cycle.root.state_path())?;
    }
    state.save(&cycle.root.state_path())?;
    Ok(report)
}

const fn rejected_status(error: &DriveError) -> Option<reqwest::StatusCode> {
    match error {
        DriveError::Rejected { status, .. } => Some(*status),
        _ => None,
    }
}

fn is_gone(error: &DriveError) -> bool {
    rejected_status(error) == Some(reqwest::StatusCode::NOT_FOUND)
}

fn is_refusal(error: &DriveError) -> bool {
    rejected_status(error).is_some_and(|status| status.is_client_error())
}

fn notify(state: &mut DriveState, rel: &str, reason: String) {
    tracing::warn!(path = %rel, %reason, "drive restored a file");
    state.delete_notices.push(DeleteNotice {
        path: rel.to_owned(),
        reason,
        at: Utc::now(),
    });
    let over = state.delete_notices.len().saturating_sub(NOTICES_KEPT);
    state.delete_notices.drain(..over);
}
