//! Dropping old snapshots and the objects only they referenced.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::cmp::Reverse;
use std::collections::HashSet;

use super::layout::Manifest;
use super::store::now_ms;
use super::{PrunePolicy, PruneReport, SnapshotError, SnapshotId, SnapshotStore};

struct Found {
    id: SnapshotId,
    created_ms: u64,
    manifest: Option<Manifest>,
}

impl SnapshotStore {
    pub fn prune(&self, policy: PrunePolicy) -> Result<PruneReport, SnapshotError> {
        let _guard = self.inner.lock.lock();
        let layout = &self.inner.layout;
        let mut found = self.found_snapshots()?;
        found.sort_by_key(|snapshot| Reverse(snapshot.created_ms));

        let now = now_ms();
        let max_age_ms = u64::try_from(policy.max_age.as_millis()).unwrap_or(u64::MAX);
        let mut keep: HashSet<String> = HashSet::new();
        let mut report = PruneReport::default();
        for (index, snapshot) in found.into_iter().enumerate() {
            let young = now.saturating_sub(snapshot.created_ms) <= max_age_ms;
            match snapshot.manifest {
                Some(manifest) if index < policy.keep_latest || young => {
                    keep.extend(manifest.files.into_values().map(|entry| entry.hash));
                },
                _ => {
                    layout.remove_manifest(&snapshot.id)?;
                    report.snapshots_removed += 1;
                },
            }
        }
        let (objects_removed, bytes_freed) = layout.collect_garbage(&keep)?;
        report.objects_removed = objects_removed;
        report.bytes_freed = bytes_freed;
        tracing::debug!(
            snapshots = report.snapshots_removed,
            objects = report.objects_removed,
            bytes = report.bytes_freed,
            "pruned snapshot store"
        );
        Ok(report)
    }

    fn found_snapshots(&self) -> Result<Vec<Found>, SnapshotError> {
        let layout = &self.inner.layout;
        let mut found = Vec::new();
        for id in layout.manifest_ids()? {
            match layout.read_manifest(&id) {
                Ok(manifest) => found.push(Found {
                    created_ms: manifest.created_ms,
                    manifest: Some(manifest),
                    id,
                }),
                Err(error) => {
                    tracing::warn!(snapshot = %id, %error, "pruning unreadable snapshot");
                    found.push(Found {
                        id,
                        created_ms: 0,
                        manifest: None,
                    });
                },
            }
        }
        Ok(found)
    }
}
