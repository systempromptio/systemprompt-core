//! Boot-time probe for a shared storage root.
//!
//! Each replica writes a marker file named after its instance id under
//! `<root>/.systemprompt/instances/`, reads it back, and lists the markers
//! left by its siblings. A root that is really shared shows sibling markers;
//! a root that only looks shared shows none.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use systemprompt_identifiers::InstanceId;
use systemprompt_traits::FileStorageError;
use tokio::fs;

const MARKER_DIR: &str = ".systemprompt/instances";

/// What the shared-mount probe observed for this replica's storage root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedMountReport {
    pub instances: Vec<String>,
    pub write_read_ok: bool,
}

impl SharedMountReport {
    #[must_use]
    pub const fn has_siblings(&self) -> bool {
        !self.instances.is_empty()
    }
}

pub async fn probe_shared_mount(
    root: &Path,
    instance_id: &InstanceId,
) -> Result<SharedMountReport, FileStorageError> {
    let marker_dir = root.join(MARKER_DIR);
    fs::create_dir_all(&marker_dir).await?;

    let marker = marker_dir.join(instance_id.as_str());
    let body = chrono::Utc::now().to_rfc3339();
    fs::write(&marker, body.as_bytes()).await?;
    let read_back = fs::read_to_string(&marker).await?;
    let write_read_ok = read_back == body;

    let mut instances = Vec::new();
    let mut entries = fs::read_dir(&marker_dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name != instance_id.as_str() {
            instances.push(name);
        }
    }
    instances.sort_unstable();

    Ok(SharedMountReport {
        instances,
        write_read_ok,
    })
}
