//! Reverting a session to an earlier message: files are restored from the
//! snapshots each step recorded, and the store's revert marker hides the later
//! messages until the next prompt deletes them or an unrevert brings them back.
//! Also the session diff and its summary.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use harness_store::{
    DiffSummary, MessageId, MessagePage, MessageWithParts, PartBody, RevertMarker, Session,
    SessionId,
};
use harness_workspace::{DiffTarget, FileDiff, SnapshotId};

use super::attachments::slash;
use crate::directory::DirState;
use crate::error::EngineError;
use crate::inner::Inner;

pub(crate) async fn commit_pending(inner: &Inner, session: &Session) -> Result<(), EngineError> {
    let Some(marker) = &session.revert else {
        return Ok(());
    };
    let id = session.id.clone();
    let from = marker.message_id.clone();
    let removed = inner
        .db
        .run(move |store| {
            let removed = store.delete_messages_from(&id, &from)?;
            store.clear_revert(&id)?;
            Ok(removed)
        })
        .await?;
    tracing::debug!(session = %session.id, removed = removed.len(), "revert committed");
    Ok(())
}

pub(crate) async fn revert(
    inner: &Arc<Inner>,
    session_id: &SessionId,
    message_id: &MessageId,
) -> Result<Session, EngineError> {
    if inner.runs.is_running(session_id) {
        return Err(EngineError::Busy {
            id: session_id.clone(),
        });
    }
    let session = inner.session(session_id).await?;
    let messages = all_messages(inner, session_id).await?;
    let index = messages
        .iter()
        .position(|message| message.message.id() == message_id)
        .ok_or_else(|| EngineError::MessageNotFound {
            session: session_id.clone(),
            id: message_id.clone(),
        })?;
    let dir = inner.dir(&session.directory).await?;

    let mut earliest: BTreeMap<PathBuf, SnapshotId> = BTreeMap::new();
    for message in &messages[index..] {
        for part in &message.parts {
            if let PartBody::Patch { hash, files } = &part.body
                && let Ok(snapshot) = hash.parse::<SnapshotId>()
            {
                for file in files {
                    earliest
                        .entry(PathBuf::from(file))
                        .or_insert_with(|| snapshot.clone());
                }
            }
        }
    }

    let (restore_point, mut touched) = match session.revert.as_ref().and_then(previous_state) {
        Some(previous) => previous,
        None => (dir.workspace.track().await?, BTreeSet::new()),
    };
    let mut groups: BTreeMap<SnapshotId, Vec<PathBuf>> = BTreeMap::new();
    for (file, snapshot) in earliest {
        touched.insert(slash(&file));
        groups.entry(snapshot).or_default().push(file);
    }
    for (snapshot, files) in groups {
        let restored = dir.workspace.revert_files(snapshot, files).await?;
        tracing::debug!(session = %session_id, files = restored.len(), "files reverted");
    }

    let marker = RevertMarker {
        message_id: message_id.clone(),
        part_id: None,
        snapshot: Some(restore_point.to_string()),
        diff: serde_json::to_string(&touched).ok(),
    };
    let key = session_id.clone();
    let updated = inner
        .db
        .run(move |store| store.set_revert(&key, marker))
        .await?;
    refresh_summary(inner, session_id, &dir).await;
    Ok(updated)
}

pub(crate) async fn unrevert(
    inner: &Arc<Inner>,
    session_id: &SessionId,
) -> Result<Session, EngineError> {
    if inner.runs.is_running(session_id) {
        return Err(EngineError::Busy {
            id: session_id.clone(),
        });
    }
    let session = inner.session(session_id).await?;
    let marker = session
        .revert
        .as_ref()
        .ok_or(EngineError::NothingToRevert)?;
    let dir = inner.dir(&session.directory).await?;
    if let Some((snapshot, files)) = previous_state(marker) {
        let files: Vec<PathBuf> = files.into_iter().map(PathBuf::from).collect();
        if !files.is_empty() {
            dir.workspace.revert_files(snapshot, files).await?;
        }
    }
    let key = session_id.clone();
    let updated = inner.db.run(move |store| store.clear_revert(&key)).await?;
    refresh_summary(inner, session_id, &dir).await;
    Ok(updated)
}

fn previous_state(marker: &RevertMarker) -> Option<(SnapshotId, BTreeSet<String>)> {
    let snapshot = marker.snapshot.as_deref()?.parse::<SnapshotId>().ok()?;
    let files = marker
        .diff
        .as_deref()
        .and_then(|text| serde_json::from_str(text).ok())
        .unwrap_or_default();
    Some((snapshot, files))
}

pub(crate) async fn diff(
    inner: &Arc<Inner>,
    session_id: &SessionId,
) -> Result<Vec<FileDiff>, EngineError> {
    let session = inner.session(session_id).await?;
    let dir = inner.dir(&session.directory).await?;
    let messages = all_messages(inner, session_id).await?;
    session_diff(&dir, &messages, session.revert.as_ref()).await
}

pub(crate) async fn refresh_summary(inner: &Inner, session_id: &SessionId, dir: &DirState) {
    let key = session_id.clone();
    let read = inner
        .db
        .run(move |store| store.messages(&key, &MessagePage::all()))
        .await;
    let messages = match read {
        Ok(messages) => messages,
        Err(error) => {
            tracing::debug!(session = %session_id, error = %error, "diff summary input unreadable");
            return;
        },
    };
    let revert = inner
        .session(session_id)
        .await
        .ok()
        .and_then(|session| session.revert);
    let diffs = match session_diff(dir, &messages, revert.as_ref()).await {
        Ok(diffs) => diffs,
        Err(error) => {
            tracing::debug!(session = %session_id, error = %error, "diff summary not computed");
            return;
        },
    };
    let totals = harness_workspace::DiffSummary::from_diffs(&diffs);
    let summary = DiffSummary {
        additions: u64::try_from(totals.additions).unwrap_or(u64::MAX),
        deletions: u64::try_from(totals.deletions).unwrap_or(u64::MAX),
        files: u64::try_from(totals.files).unwrap_or(u64::MAX),
    };
    let key = session_id.clone();
    if let Err(error) = inner
        .db
        .run(move |store| store.set_summary(&key, summary))
        .await
    {
        tracing::debug!(session = %session_id, error = %error, "could not store the diff summary");
    }
}

async fn session_diff(
    dir: &DirState,
    messages: &[MessageWithParts],
    revert: Option<&RevertMarker>,
) -> Result<Vec<FileDiff>, EngineError> {
    let visible = messages
        .iter()
        .filter(|message| revert.is_none_or(|marker| message.message.id() < &marker.message_id));
    let mut first: Option<SnapshotId> = None;
    let mut files: BTreeSet<String> = BTreeSet::new();
    for message in visible {
        for part in &message.parts {
            match &part.body {
                PartBody::StepStart {
                    snapshot: Some(snapshot),
                } if first.is_none() => {
                    first = snapshot.parse::<SnapshotId>().ok();
                },
                PartBody::Patch { files: changed, .. } => files.extend(changed.iter().cloned()),
                _ => {},
            }
        }
    }
    let Some(first) = first else {
        return Ok(Vec::new());
    };
    if files.is_empty() {
        return Ok(Vec::new());
    }
    let root = dir.workspace.root().to_path_buf();
    let diffs = dir.workspace.diff(first, DiffTarget::Working).await?;
    Ok(diffs
        .into_iter()
        .filter(|diff| files.contains(&absolute(&root, &diff.path)))
        .collect())
}

fn absolute(root: &Path, relative: &Path) -> String {
    slash(&root.join(relative))
}

async fn all_messages(
    inner: &Inner,
    session_id: &SessionId,
) -> Result<Vec<MessageWithParts>, EngineError> {
    let key = session_id.clone();
    inner
        .db
        .run(move |store| store.messages(&key, &MessagePage::all()))
        .await
}
