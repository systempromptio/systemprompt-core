//! The bridge host-filter write `admin config migrate --data` makes.
//!
//! The `oauth` member owns the per-host model-protocol filters and provides
//! this port **sealed** (`DynHostFilterMigrationPort`): the kernel's
//! `config migrate` command reads it, a member's command, job or route
//! cannot. It is held as `Arc<dyn _>`, hence `#[async_trait]` for `dyn`
//! dispatch.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;

use crate::repository::RepositoryError;

/// Rewrites one model-protocol tag in every stored host filter; idempotent,
/// returning the number of filters changed.
#[async_trait]
pub trait HostFilterMigrationPort: Send + Sync {
    async fn rewrite_model_protocol_tag(
        &self,
        from: &str,
        to: &str,
    ) -> Result<u64, RepositoryError>;
}

pub type DynHostFilterMigrationPort = Arc<dyn HostFilterMigrationPort>;
