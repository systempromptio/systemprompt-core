//! Owner reassignment across domain repositories.
//!
//! Merging one user into another touches every domain's tables. Each domain
//! implements this trait over its own rows; the app-layer merge orchestrator
//! holds the implementations as `Arc<dyn OwnerReassignment>` and runs them in
//! order, so `#[async_trait]` is required for `dyn` dispatch. No domain reads
//! another domain's tables to do this.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use std::sync::Arc;
use systemprompt_identifiers::UserId;

use crate::repository::RepositoryError;

/// Rows a domain reassigned from one owner to another, per table.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ReassignedRows {
    pub tables: Vec<(&'static str, u64)>,
}

impl ReassignedRows {
    #[must_use]
    pub fn total(&self) -> u64 {
        self.tables.iter().map(|(_, n)| *n).sum()
    }
}

/// Reassigns every row a domain holds for `from` to `to`.
///
/// Implemented by each domain over its own tables and dispatched as
/// `Arc<dyn OwnerReassignment>` by the merge orchestrator.
#[async_trait]
pub trait OwnerReassignment: Send + Sync {
    fn domain(&self) -> &'static str;

    async fn reassign_owner(
        &self,
        from: &UserId,
        to: &UserId,
    ) -> Result<ReassignedRows, RepositoryError>;
}

pub type DynOwnerReassignment = Arc<dyn OwnerReassignment>;
