//! The per-session comms inbox: where announcements land, and the recovery
//! of a drain that was interrupted between the rename and the read.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io::Write;
use std::path::{Path, PathBuf};

use super::CommsAnnouncement;

pub const INBOX_DIR_NAME: &str = "inbox";
pub const DRAINING_SUFFIX: &str = ".draining";

#[must_use]
pub fn inbox_dir() -> Option<PathBuf> {
    crate::basedirs::config_dir().map(|d| {
        d.join(crate::brand::brand().config_dir)
            .join(INBOX_DIR_NAME)
    })
}

#[must_use]
pub fn inbox_path(session_id: &crate::ids::HookSessionId) -> Option<PathBuf> {
    let safe: String = session_id
        .as_str()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    if safe.is_empty() {
        return None;
    }
    inbox_dir().map(|d| d.join(format!("{safe}.jsonl")))
}

#[derive(Debug, thiserror::Error)]
pub enum InboxError {
    #[error("announcement {message_id} names no session")]
    NoSession {
        message_id: crate::ids::CommsMessageId,
    },
    #[error("announcement {message_id} has an unusable session id")]
    UnusableSession {
        message_id: crate::ids::CommsMessageId,
    },
    #[error("no config directory for the comms inbox")]
    NoConfigDir,
    #[error("serialise announcement {message_id}: {source}")]
    Serialize {
        message_id: crate::ids::CommsMessageId,
        #[source]
        source: serde_json::Error,
    },
    #[error("{action} {path}: {source}")]
    Io {
        action: &'static str,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

// Why: a crash between renaming the inbox to `.draining` and reading it back
// would otherwise strand the messages; on start every leftover is folded
// back into the live inbox before any new announcement lands.
pub fn sweep_draining() -> Result<usize, InboxError> {
    let dir = inbox_dir().ok_or(InboxError::NoConfigDir)?;
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(source) => {
            return Err(InboxError::Io {
                action: "enumerate",
                path: dir,
                source,
            });
        },
    };
    let mut restored = 0;
    for entry in entries {
        let entry = entry.map_err(|source| InboxError::Io {
            action: "enumerate",
            path: dir.clone(),
            source,
        })?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(stem) = name.strip_suffix(DRAINING_SUFFIX) else {
            continue;
        };
        let Some(session_stem) = stem.split_once(".jsonl.").map(|(s, _)| s) else {
            continue;
        };
        let target = dir.join(format!("{session_stem}.jsonl"));
        let body = std::fs::read(entry.path()).map_err(|source| InboxError::Io {
            action: "read",
            path: entry.path(),
            source,
        })?;
        append_bytes(&target, &body)?;
        std::fs::remove_file(entry.path()).map_err(|source| InboxError::Io {
            action: "remove",
            path: entry.path(),
            source,
        })?;
        restored += 1;
    }
    Ok(restored)
}

fn append_bytes(path: &Path, bytes: &[u8]) -> Result<(), InboxError> {
    let parent = path.parent().ok_or(InboxError::NoConfigDir)?;
    crate::fsutil::create_dir_all_mode_0700(parent).map_err(|source| InboxError::Io {
        action: "create",
        path: parent.to_path_buf(),
        source,
    })?;
    let mut file = crate::fsutil::open_append_0600(path).map_err(|source| InboxError::Io {
        action: "open",
        path: path.to_path_buf(),
        source,
    })?;
    file.write_all(bytes).map_err(|source| InboxError::Io {
        action: "append",
        path: path.to_path_buf(),
        source,
    })
}

pub(super) fn append(announcement: &CommsAnnouncement) -> Result<(), InboxError> {
    let message_id = announcement.message_id.clone();
    let session_id = announcement
        .session_id
        .as_ref()
        .ok_or_else(|| InboxError::NoSession {
            message_id: message_id.clone(),
        })?;
    let path = inbox_path(session_id).ok_or_else(|| InboxError::UnusableSession {
        message_id: message_id.clone(),
    })?;
    let mut line = serde_json::to_vec(announcement)
        .map_err(|source| InboxError::Serialize { message_id, source })?;
    line.push(b'\n');
    append_bytes(&path, &line)
}
