//! The local `.trash` folder: where a remote deletion puts the synced copy,
//! and how long it stays there.
//!
//! Each deletion lands under `.trash/<UTC timestamp>/<path>`. A stamp folder
//! older than [`RETENTION_DAYS`] is removed at the start of a cycle; a folder
//! whose name is not a stamp this module wrote is never touched.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

use chrono::{DateTime, NaiveDateTime, Utc};

use super::error::{DriveError, DriveResult};
use super::root::DriveRoot;
use super::write::move_file;

pub const RETENTION_DAYS: i64 = 30;
const STAMP: &str = "%Y%m%dT%H%M%S%.3fZ";

pub fn move_to_trash(
    root: &DriveRoot,
    rel: &str,
    now: DateTime<Utc>,
) -> DriveResult<Option<PathBuf>> {
    let source = root.file(rel)?;
    if !source.exists() {
        return Ok(None);
    }
    let target = root
        .trash_dir()
        .join(now.format(STAMP).to_string())
        .join(rel);
    move_file(&source, &target)?;
    Ok(Some(target))
}

#[must_use]
pub fn stamp_of(name: &str) -> Option<DateTime<Utc>> {
    NaiveDateTime::parse_from_str(name, STAMP)
        .ok()
        .map(|naive| naive.and_utc())
}

pub fn prune(root: &DriveRoot, now: DateTime<Utc>) -> DriveResult<usize> {
    let dir = root.trash_dir();
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(DriveError::io("read drive trash", &dir, e)),
    };
    let cutoff = now - chrono::Duration::days(RETENTION_DAYS);
    let mut removed = 0;
    for entry in entries {
        let entry = entry.map_err(|e| DriveError::io("read drive trash", &dir, e))?;
        let expired = entry
            .file_name()
            .to_str()
            .and_then(stamp_of)
            .is_some_and(|stamp| stamp < cutoff);
        let is_dir = entry
            .file_type()
            .map_err(|e| DriveError::io("read drive trash", entry.path(), e))?
            .is_dir();
        if expired && is_dir {
            std::fs::remove_dir_all(entry.path())
                .map_err(|e| DriveError::io("prune drive trash", entry.path(), e))?;
            removed += 1;
        }
    }
    Ok(removed)
}
