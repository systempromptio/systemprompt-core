//! Placing one document's head in the Drive folder, moving a known file to the
//! path the instance names, and keeping a conflicting local edit beside the
//! head.
//!
//! A local file with edits the index does not know of is never overwritten: it
//! is kept beside as a conflict copy first, and the local file is hashed again
//! just before the head replaces it, so an edit made during the download
//! is kept too. The local path of a known document is the one the index
//! records.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use chrono::Utc;
use systemprompt_identifiers::{DocumentId, DocumentRevisionId};

use super::cycle::{Cycle, Report};
use super::disk::{
    Guarded, free_conflict_sibling, move_file, synced_entry, write_unless_changed, write_verified,
};
use super::error::SyncError;
use super::index::{ConflictNotice, SyncIndex};
use super::names::{PathRefusal, case_collision, fold};
use super::root::SyncRoot;
use super::scan::{hash, metadata};
use super::status::ActivityKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HeadRef {
    pub(crate) document_id: DocumentId,
    pub(crate) revision_id: DocumentRevisionId,
    pub(crate) sha256: String,
}

#[derive(Debug, Clone)]
pub(crate) struct Fetched<'a> {
    pub(crate) head: HeadRef,
    pub(crate) bytes: &'a [u8],
}

pub(crate) async fn move_and_place(
    cx: Cycle<'_>,
    index: &mut SyncIndex,
    rel: &str,
    head: HeadRef,
    report: &mut Report,
) -> Result<(), SyncError> {
    let document_id = head.document_id.clone();
    if let Some(known) = index.path_of(&document_id).map(str::to_owned)
        && known != rel
    {
        rename(cx, index, &known, rel, &document_id)?;
        report.note(rel, ActivityKind::Moved { from: known });
    }
    place_head(cx, index, rel, head, report).await?;
    index.clear_unplaced(&document_id);
    Ok(())
}

async fn place_head(
    cx: Cycle<'_>,
    index: &mut SyncIndex,
    rel: &str,
    head: HeadRef,
    report: &mut Report,
) -> Result<(), SyncError> {
    let path = cx.root.file(rel)?;
    if index.pending_deletes.contains_key(rel) && !path.exists() {
        return Ok(());
    }
    ensure_no_collision(cx.root, index, rel)?;
    let known = index.entries.get(rel).cloned();
    let in_step = known
        .as_ref()
        .is_some_and(|entry| entry.revision_id == head.revision_id && entry.sha256 == head.sha256);
    if in_step && path.exists() {
        return Ok(());
    }
    let local = if path.exists() {
        Some(hash(&path)?)
    } else {
        None
    };
    if local.as_deref() == Some(head.sha256.as_str()) {
        adopt(index, &path, rel, head)?;
        return Ok(());
    }
    let bytes = cx.feed.blob(cx.token, &head.sha256).await?;
    let unsynced_edit = local
        .as_ref()
        .is_some_and(|sha| known.as_ref().is_none_or(|entry| &entry.sha256 != sha));
    let written = !unsynced_edit
        && write_unless_changed(cx.root, &path, &bytes, &head.sha256, local.as_deref())?
            == Guarded::Written;
    if written {
        adopt(index, &path, rel, head)?;
        report.note(rel, ActivityKind::Downloaded);
    } else {
        keep_local_beside(
            cx,
            index,
            rel,
            Fetched {
                head,
                bytes: &bytes,
            },
            report,
        )?;
    }
    Ok(())
}

fn adopt(index: &mut SyncIndex, path: &Path, rel: &str, head: HeadRef) -> Result<(), SyncError> {
    let HeadRef {
        document_id,
        revision_id,
        sha256,
    } = head;
    let entry = synced_entry(path, Some(document_id), revision_id, sha256)?;
    index.entries.insert(rel.to_owned(), entry);
    Ok(())
}

pub(crate) fn keep_local_beside(
    cx: Cycle<'_>,
    index: &mut SyncIndex,
    rel: &str,
    fetched: Fetched<'_>,
    report: &mut Report,
) -> Result<ConflictNotice, SyncError> {
    let Fetched { head, bytes } = fetched;
    let original = cx.root.file(rel)?;
    let sibling = free_conflict_sibling(cx.root, rel, cx.host)?;
    let sibling_path = cx.root.file(&sibling)?;
    let moved = original.exists();
    if moved {
        move_file(cx.root, &original, &sibling_path)?;
    }
    if let Err(error) = write_verified(cx.root, &original, bytes, &head.sha256) {
        if moved && let Err(restore) = move_file(cx.root, &sibling_path, &original) {
            tracing::error!(error = %restore, path = %rel, "a local edit could not be put back after a failed write");
        }
        return Err(error);
    }
    index.clear_unplaced(&head.document_id);
    let notice = ConflictNotice {
        document_id: Some(head.document_id.clone()),
        unplaced: false,
        path: rel.to_owned(),
        sibling: sibling.clone(),
        head_revision_id: head.revision_id.clone(),
        detected_at: Utc::now(),
    };
    let entry = synced_entry(
        &original,
        Some(head.document_id),
        head.revision_id,
        head.sha256,
    )?;
    index.entries.insert(rel.to_owned(), entry);
    index.push_conflict(&notice);
    index.save(&cx.root.index_path())?;
    tracing::warn!(path = %rel, sibling = %sibling, "the Drive sync kept a conflicting local edit beside the head");
    report.note(rel, ActivityKind::Conflict { kept_as: sibling });
    Ok(notice)
}

pub(crate) fn rename(
    cx: Cycle<'_>,
    index: &mut SyncIndex,
    from: &str,
    to: &str,
    document_id: &DocumentId,
) -> Result<(), SyncError> {
    let source = cx.root.file(from)?;
    let target = cx.root.file(to)?;
    let case_only = fold(from) == fold(to);
    if !case_only {
        ensure_no_collision(cx.root, index, to)?;
        if target.exists() && !index.entries.contains_key(to) {
            return Err(SyncError::Unplaceable {
                path: to.to_owned(),
                refusal: PathRefusal::Occupied,
            });
        }
    }
    if source.exists() && (case_only || !target.exists()) {
        move_file(cx.root, &source, &target)?;
    }
    if let Some(mut entry) = index.entries.remove(from) {
        entry.document_id = Some(document_id.clone());
        if target.exists() {
            let (mtime_unix, size) = metadata(&target)?;
            entry.mtime_unix = mtime_unix;
            entry.size = size;
        }
        index.entries.insert(to.to_owned(), entry);
    }
    Ok(())
}

fn ensure_no_collision(root: &SyncRoot, index: &SyncIndex, rel: &str) -> Result<(), SyncError> {
    let refuse = |refusal| SyncError::Unplaceable {
        path: rel.to_owned(),
        refusal,
    };
    case_collision(rel, index.entries.keys().map(String::as_str)).map_err(refuse)?;
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
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => return Err(SyncError::io("list", &parent, error)),
    };
    case_collision(rel, listed.iter().map(String::as_str)).map_err(refuse)
}
