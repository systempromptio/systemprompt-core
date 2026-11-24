//! The bridge host-filter rewrite `admin config migrate --data` runs.
//!
//! The `oauth` member implements it over its bridge host preference table and
//! provides it **sealed** (`DynBridgeHostFilterMigration`): the kernel command
//! reads it, a member's command, job or route cannot. It is held as
//! `Arc<dyn _>`, hence `#[async_trait]` for `dyn` dispatch.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;

use crate::repository::RepositoryError;

/// Renames one model-protocol tag in every bridge host filter that carries
/// it, returning how many filters changed.
///
/// `#[async_trait]` keeps the trait dyn-compatible: it is held as
/// `DynBridgeHostFilterMigration`.
#[async_trait]
pub trait BridgeHostFilterMigration: Send + Sync {
    async fn rename_model_protocol_tag(&self, from: &str, to: &str)
    -> Result<u64, RepositoryError>;
}

pub type DynBridgeHostFilterMigration = Arc<dyn BridgeHostFilterMigration>;
