//! The guards a pulled change passes before it touches the Drive folder.
//!
//! A remote deletion acts only on the path this device's state records for
//! the document; the path the feed names is never consulted, so the gateway
//! cannot address a file this device never synced. A file that still holds
//! the synced bytes goes to [`super::trash`]; one edited since is renamed to
//! a conflict sibling and kept, and its entry is dropped so the next upload
//! offers it as a new document.
//!
//! A path that cannot be placed ([`super::names::PathRefusal`]) does not stop
//! the feed: it is kept as a conflict, naming the file that already holds
//! the name for a case collision, or the path itself otherwise. That conflict
//! stays until the name becomes placeable or a later change for the same
//! document is applied.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::Utc;
use systemprompt_identifiers::{DocumentId, DocumentRevisionId};

use super::Cycle;
use super::deletes::record_notice;
use super::error::{DriveError, DriveResult};
use super::hash::hash_file;
use super::names::{PathRefusal, case_collision};
use super::pull::PullReport;
use super::root::DriveRoot;
use super::state::{ConflictNotice, DriveState};
use super::write::{free_conflict_sibling, move_file};

pub fn remote_delete(
    cycle: Cycle<'_>,
    state: &mut DriveState,
    document_id: &DocumentId,
    report: &mut PullReport,
) -> DriveResult<()> {
    let Some(rel) = state.path_of(document_id).map(str::to_owned) else {
        tracing::debug!(%document_id, "drive ignored the deletion of a document this device never synced");
        return Ok(());
    };
    let synced = state.entries[&rel].clone();
    let path = cycle.root.file(&rel)?;
    if path.exists() && hash_file(&path)?.0 != synced.sha256 {
        let sibling = free_conflict_sibling(cycle.root, &rel, cycle.host_name)?;
        move_file(&path, &cycle.root.file(&sibling)?)?;
        let notice = record_conflict(state, document_id, &rel, sibling, synced.revision_id);
        tracing::warn!(path = %rel, sibling = %notice.sibling, "drive kept a local edit to a document deleted on the server");
        report.conflicts.push(notice);
    } else if let Some(trashed) = super::trash::move_to_trash(cycle.root, &rel, Utc::now())? {
        tracing::info!(path = %rel, trashed = %trashed.display(), "drive moved a deleted document to the trash");
    }
    state.entries.remove(&rel);
    state.pending_deletes.remove(&rel);
    report.applied += 1;
    Ok(())
}

pub fn ensure_no_collision(root: &DriveRoot, state: &DriveState, rel: &str) -> DriveResult<()> {
    let refuse = |refusal| DriveError::Unplaceable {
        path: rel.to_owned(),
        refusal,
    };
    case_collision(rel, state.entries.keys().map(String::as_str)).map_err(refuse)?;
    let (dir, _) = rel.rsplit_once('/').unwrap_or(("", rel));
    let parent = if dir.is_empty() {
        root.dir().to_path_buf()
    } else {
        root.file(dir)?
    };
    let listed: Vec<String> = match std::fs::read_dir(&parent) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .filter_map(|entry| entry.file_name().into_string().ok())
            .map(|name| {
                if dir.is_empty() {
                    name
                } else {
                    format!("{dir}/{name}")
                }
            })
            .collect(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(DriveError::io("list drive folder", &parent, e)),
    };
    case_collision(rel, listed.iter().map(String::as_str)).map_err(refuse)
}

/// A change whose path could not be placed.
#[derive(Debug, Clone, Copy)]
pub struct Unplaced<'a> {
    pub document_id: &'a DocumentId,
    pub rel: &'a str,
    pub refusal: &'a PathRefusal,
    pub revision_id: Option<&'a DocumentRevisionId>,
}

pub fn skip_unplaceable(state: &mut DriveState, unplaced: Unplaced<'_>, report: &mut PullReport) {
    let Unplaced {
        document_id,
        rel,
        refusal,
        revision_id,
    } = unplaced;
    tracing::warn!(path = %rel, %refusal, "drive did not place a document");
    let revision_id = revision_id.cloned().or_else(|| {
        state
            .path_of(document_id)
            .and_then(|known| state.entries.get(known))
            .map(|entry| entry.revision_id.clone())
    });
    let Some(revision_id) = revision_id else {
        record_notice(state, rel, format!("not placed: {refusal}"));
        return;
    };
    let sibling = match refusal {
        PathRefusal::CaseCollision { existing } => existing.clone(),
        _ => rel.to_owned(),
    };
    let notice = record_conflict(state, document_id, rel, sibling, revision_id);
    report.conflicts.push(notice);
}

fn record_conflict(
    state: &mut DriveState,
    document_id: &DocumentId,
    rel: &str,
    sibling: String,
    revision_id: DocumentRevisionId,
) -> ConflictNotice {
    let notice = ConflictNotice {
        document_id: Some(document_id.clone()),
        path: rel.to_owned(),
        sibling,
        head_revision_id: revision_id,
        detected_at: Utc::now(),
    };
    if !state
        .conflicts
        .iter()
        .any(|known| known.path == notice.path && known.sibling == notice.sibling)
    {
        state.conflicts.push(notice.clone());
    }
    notice
}
