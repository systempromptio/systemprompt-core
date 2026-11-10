//! The guards a pulled change passes before it touches the Drive folder.
//!
//! A remote deletion acts only on the path this device's state records for
//! the document; the path the feed names is never consulted, so the gateway
//! cannot address a file this device never synced. A file that still holds
//! the synced bytes goes to [`super::trash`]; one edited since is renamed to
//! a conflict sibling and kept, and its entry is dropped so the next upload
//! offers it as a new document.
//!
//! A path that cannot be placed ([`super::names::PathRefusal`]) is skipped
//! with a notice instead of stopping the feed; a case collision is kept as a
//! conflict naming the file that already holds the name.
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
        let notice = record_conflict(state, &rel, sibling, synced.revision_id);
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

pub fn skip_unplaceable(
    state: &mut DriveState,
    rel: &str,
    refusal: &PathRefusal,
    revision_id: Option<&DocumentRevisionId>,
    report: &mut PullReport,
) {
    tracing::warn!(path = %rel, %refusal, "drive did not place a document");
    match (refusal, revision_id) {
        (PathRefusal::CaseCollision { existing }, Some(revision_id)) => {
            let notice = record_conflict(state, rel, existing.clone(), revision_id.clone());
            report.conflicts.push(notice);
        },
        _ => record_notice(state, rel, format!("not placed: {refusal}")),
    }
}

fn record_conflict(
    state: &mut DriveState,
    rel: &str,
    sibling: String,
    revision_id: DocumentRevisionId,
) -> ConflictNotice {
    let notice = ConflictNotice {
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
