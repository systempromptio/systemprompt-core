//! Scheduled maintenance for the document store.
//!
//! [`DocumentBlobGcJob`] deletes blobs no revision or edit session has
//! referenced for the configured grace period; [`DocumentPresenceSweepJob`]
//! removes expired presence rows and write acknowledgements past their
//! seven-day replay window; [`DocumentCollabUpdateSweepJob`] deletes the
//! update log of closed edit sessions; [`DocumentTrashPurgeJob`] hard-deletes
//! documents trashed longer than the retention period;
//! [`DocumentNodeIdsBackfillJob`] mints node ids for revisions written before
//! ids existed, once per release. All are cluster-scoped
//! and registered with `submit_job!`, as the files domain registers its other
//! jobs. Each run builds its repositories from the job context's `DbPool` and
//! takes file storage from the context's `ExtensionContext`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod blob_gc;
mod collab_update_sweep;
mod node_ids_backfill;
mod presence_sweep;
mod trash_purge;

pub use blob_gc::DocumentBlobGcJob;
pub use collab_update_sweep::DocumentCollabUpdateSweepJob;
pub use node_ids_backfill::{BackfillReport, DocumentNodeIdsBackfillJob};
pub use presence_sweep::DocumentPresenceSweepJob;
pub use trash_purge::DocumentTrashPurgeJob;
