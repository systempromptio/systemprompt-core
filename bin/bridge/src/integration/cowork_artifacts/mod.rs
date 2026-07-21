//! Cowork desktop artifacts integration: materialises the manifest's
//! `artifacts` section into the Cowork global Artifacts-library store.
//!
//! Unlike [`super::cowork_plugins`] (a settings toggle), this emitter writes
//! content, so it follows the synthetic-plugin writer's shape: a content-hashed
//! `version.json` written last as the idempotency/completion marker. Teardown
//! is explicit: only [`HostSync::clear`] (the disabled-host path) removes the
//! store. An enabled host sending an empty artifact set preserves the store and
//! warns — that shape signals an upstream population bug, not intent to wipe.
//! The write mechanism itself is pluggable (see
//! [`sink`]): the live Cowork library ingests artifacts only via its native
//! `create_artifact` tool, so [`emit::active_sinks`] writes through both
//! [`sink::SeedStaging`] (input for the first-run seed skill) and
//! [`sink::FileSink`] (GUI listing + future directly-writable library).
//!
//! The emitter reuses the `"cowork"` host id, so it fires whenever Cowork is in
//! the manifest's `enabled_hosts` — the same gate as the plugin emitter.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod emit;
pub mod sink;

use async_trait::async_trait;

use crate::sync::apply::ApplyError;
use crate::sync::host_sync::{HostSync, HostSyncCtx};

#[derive(Clone, Copy, Debug)]
pub struct CoworkArtifactsSync;

#[async_trait]
impl HostSync for CoworkArtifactsSync {
    fn host_id(&self) -> &'static str {
        "cowork"
    }

    async fn apply(&self, ctx: &HostSyncCtx<'_>) -> Result<(), ApplyError> {
        let Some(dir) = emit::resolve_artifacts_dir() else {
            return Ok(());
        };
        emit::write_artifacts(&dir, emit::active_sinks(), &ctx.manifest.artifacts)
    }

    fn clear(&self) -> Result<(), ApplyError> {
        let Some(dir) = emit::resolve_artifacts_dir() else {
            return Ok(());
        };
        emit::remove_dir(&dir)
    }
}
