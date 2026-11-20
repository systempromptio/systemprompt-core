//! Owner reassignment across domain repositories.
//!
//! Merging one user into another touches every domain's tables. Each owning
//! member implements this trait over its own rows and contributes it; the
//! kernel admits a contribution only when every [`ReassignedColumn`] it
//! names is a column of a table the contributing member's own schema
//! creates, and the users domain runs the admitted set in composition order.
//! `#[async_trait]` is required for `dyn` dispatch. No member reassigns
//! another member's tables.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use std::sync::Arc;
use systemprompt_identifiers::UserId;

use crate::repository::RepositoryError;

/// One user-keyed column a reassignment rewrites.
///
/// Both names are plain `PostgreSQL` identifiers (an ASCII letter or `_`,
/// then ASCII letters, digits or `_`, at most 63 bytes, checked by
/// `is_plain_identifier`); admission refuses any other spelling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ReassignedColumn {
    pub table: &'static str,
    pub column: &'static str,
}

#[must_use]
pub const fn is_plain_identifier(name: &str) -> bool {
    let bytes = name.as_bytes();
    if bytes.is_empty() || bytes.len() > 63 {
        return false;
    }
    if !(bytes[0].is_ascii_alphabetic() || bytes[0] == b'_') {
        return false;
    }
    let mut index = 1;
    while index < bytes.len() {
        if !(bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_') {
            return false;
        }
        index += 1;
    }
    true
}

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
/// Implemented by each owning member over its own tables, contributed with
/// `Contributions::owner_reassignment` and dispatched as
/// `Arc<dyn OwnerReassignment>` by the users domain's merge. [`columns`]
/// names every column its statements rewrite or delete by.
///
/// [`columns`]: OwnerReassignment::columns
#[async_trait]
pub trait OwnerReassignment: Send + Sync {
    fn domain(&self) -> &'static str;

    fn columns(&self) -> &'static [ReassignedColumn];

    async fn reassign_owner(
        &self,
        from: &UserId,
        to: &UserId,
    ) -> Result<ReassignedRows, RepositoryError>;
}

pub type DynOwnerReassignment = Arc<dyn OwnerReassignment>;
