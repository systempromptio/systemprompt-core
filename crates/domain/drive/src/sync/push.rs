//! Committing local edits as revisions.
//!
//! Each changed file is read once, hashed from the bytes actually sent, and
//! committed with the revision it was based on. A committed revision becomes
//! the file's index entry. A merged one (the instance landed the edit on a
//! newer head) is recorded against the uploaded bytes, so the next pull brings
//! the merged content. A conflict keeps the local file beside the head as a
//! conflict copy. A proposed edit (this device may only suggest on that
//! document) is kept as edited and recorded as sent for review, so it is not
//! sent again. A file that changed again during its upload is left for the next
//! cycle. A file the instance refuses is reported and tried again next cycle
//! without stopping the others.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use bytes::Bytes;
use chrono::Utc;
use sp_protocol::ids::DocumentRevisionId;
use sp_protocol::wire::drive::{CommitMeta, CommitOutcome};

use super::cycle::{Cycle, Report};
use super::error::SyncError;
use super::index::{IndexEntry, SentForReview, SyncIndex};
use super::place::{Fetched, HeadRef, keep_local_beside};
use super::scan::{LocalChange, hash, local_changes, metadata};
use super::status::ActivityKind;
use crate::hash::sha256_hex;

pub(crate) async fn upload(
    cx: Cycle<'_>,
    index: &mut SyncIndex,
    report: &mut Report,
) -> Result<(), SyncError> {
    for change in local_changes(cx.root, index)? {
        let path = change.file.rel.clone();
        match upload_one(cx, index, change, report).await {
            Ok(()) => {},
            Err(error) if error.is_refusal() => {
                tracing::warn!(path = %path, error = %error, "the instance refused an upload; the other files carry on");
                let reason = error.to_string();
                report.note(
                    &path,
                    ActivityKind::Refused {
                        reason: reason.clone(),
                    },
                );
                report.refused.push((path, reason));
            },
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

async fn upload_one(
    cx: Cycle<'_>,
    index: &mut SyncIndex,
    change: LocalChange,
    report: &mut Report,
) -> Result<(), SyncError> {
    let LocalChange { file, base, .. } = change;
    let path = file.abs;
    let rel = file.rel;
    let bytes =
        std::fs::read(&path).map_err(|error| SyncError::io("read for upload", &path, error))?;
    let sha256 = sha256_hex(&bytes);
    let size = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    let meta = CommitMeta {
        document_id: base.as_ref().and_then(|base| base.document_id.clone()),
        path: rel.clone(),
        base_revision: base.map(|base| base.revision_id),
        content_sha256: sha256.clone(),
        size_bytes: size,
        device_id: cx.device_id.clone(),
        message: None,
    };
    let outcome = cx.feed.commit(cx.token, &meta, Bytes::from(bytes)).await?;
    match outcome {
        CommitOutcome::Committed { revision_id } => {
            let unchanged = record(index, &path, &meta, revision_id)?;
            if !unchanged {
                tracing::info!(path = %rel, "a file changed during its upload; the next cycle commits it");
            }
            index.save(&cx.root.index_path())?;
            report.note(&rel, ActivityKind::Uploaded);
        },
        CommitOutcome::Merged { revision_id, .. } => {
            record(index, &path, &meta, revision_id)?;
            index.save(&cx.root.index_path())?;
            report.note(&rel, ActivityKind::Merged);
        },
        CommitOutcome::Proposed {
            proposal_id,
            revision_id,
        } => {
            let (mtime_unix, _) = metadata(&path)?;
            let entry = IndexEntry {
                document_id: meta.document_id.clone(),
                revision_id: meta.base_revision.clone().unwrap_or(revision_id),
                sha256,
                mtime_unix,
                size,
            };
            index.entries.insert(rel.clone(), entry);
            index.sent_for_review.retain(|sent| sent.path != rel);
            index.sent_for_review.push(SentForReview {
                path: rel.clone(),
                proposal_id,
                sent_at: Utc::now(),
            });
            index.save(&cx.root.index_path())?;
            tracing::info!(path = %rel, "a local edit went to the instance for review; the file is kept as edited");
            report.note(&rel, ActivityKind::SentForReview);
        },
        CommitOutcome::Conflict {
            head_revision_id, ..
        } => {
            resolve_conflict(cx, index, &rel, &head_revision_id, report).await?;
        },
    }
    Ok(())
}

fn record(
    index: &mut SyncIndex,
    path: &std::path::Path,
    meta: &CommitMeta,
    revision_id: DocumentRevisionId,
) -> Result<bool, SyncError> {
    let (mtime_unix, now_size) = metadata(path)?;
    let unchanged = now_size == meta.size_bytes && hash(path)? == meta.content_sha256;
    let entry = IndexEntry {
        document_id: meta.document_id.clone(),
        revision_id,
        sha256: meta.content_sha256.clone(),
        mtime_unix: if unchanged { mtime_unix } else { 0 },
        size: meta.size_bytes,
    };
    index.entries.insert(meta.path.clone(), entry);
    Ok(unchanged)
}

async fn resolve_conflict(
    cx: Cycle<'_>,
    index: &mut SyncIndex,
    rel: &str,
    head_revision_id: &DocumentRevisionId,
    report: &mut Report,
) -> Result<(), SyncError> {
    let manifest = cx.feed.manifest(cx.token).await?;
    let head = manifest
        .entries
        .into_iter()
        .find(|entry| entry.path == rel)
        .ok_or_else(|| SyncError::HeadMissing {
            path: rel.to_owned(),
        })?;
    if &head.head_revision_id != head_revision_id {
        tracing::info!(path = %rel, "the head moved again since the conflict; using the newer head");
    }
    let bytes = cx.feed.blob(cx.token, &head.content_sha256).await?;
    let head = HeadRef {
        document_id: head.document_id,
        revision_id: head.head_revision_id,
        sha256: head.content_sha256,
    };
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
    Ok(())
}
