//! Registry of the tables that hold a document blob outside the document
//! store, so blob collection keeps what they hold without naming them.
//!
//! A holder is contributed to
//! [`BlobHolderSlot`](crate::contract::BlobHolderSlot) and answers, for a
//! batch of content hashes, which of them it still references. Each holder's
//! query is a compile-time `sqlx::query!` in its own module.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;

use futures_util::future::BoxFuture;
use sqlx::PgConnection;
use systemprompt_traits::RepositoryError;

/// The hashes among the given ones that a holder still references.
pub type HeldAmong = for<'c> fn(
    &'c mut PgConnection,
    &'c [String],
) -> BoxFuture<'c, Result<Vec<String>, sqlx::Error>>;

/// One contributed holder.
#[derive(Debug, Clone, Copy)]
pub struct BlobHolder {
    pub name: &'static str,
    pub held_among: HeldAmong,
}

/// Every admitted blob holder, built by
/// [`BlobHolderSlot`](crate::contract::BlobHolderSlot).
#[derive(Debug, Clone, Default)]
pub struct BlobHolders {
    holders: Vec<BlobHolder>,
}

impl BlobHolders {
    #[must_use]
    pub const fn new(holders: Vec<BlobHolder>) -> Self {
        Self { holders }
    }

    pub(super) async fn held_among(
        &self,
        conn: &mut PgConnection,
        hashes: &[String],
    ) -> Result<HashSet<String>, RepositoryError> {
        let mut held = HashSet::new();
        if hashes.is_empty() {
            return Ok(held);
        }
        for holder in &self.holders {
            held.extend((holder.held_among)(&mut *conn, hashes).await?);
        }
        Ok(held)
    }
}
