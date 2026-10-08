//! [`OwnerReassignment`] for the files domain: moves every file row from one
//! user to another.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::UserId;
use systemprompt_traits::{OwnerReassignment, ReassignedRows, RepositoryError};

#[derive(Debug, Clone)]
pub struct FilesOwnerReassignment {
    write_pool: Arc<PgPool>,
}

impl FilesOwnerReassignment {
    pub fn new(db: &DbPool) -> Self {
        let write_pool = db.write_pool();
        Self { write_pool }
    }
}

#[async_trait]
impl OwnerReassignment for FilesOwnerReassignment {
    fn domain(&self) -> &'static str {
        "files"
    }

    async fn reassign_owner(
        &self,
        from: &UserId,
        to: &UserId,
    ) -> Result<ReassignedRows, RepositoryError> {
        let files = sqlx::query!(
            "UPDATE files SET user_id = $2 WHERE user_id = $1",
            from.as_str(),
            to.as_str()
        )
        .execute(self.write_pool.as_ref())
        .await
        .map_err(RepositoryError::database)?
        .rows_affected();

        Ok(ReassignedRows {
            tables: vec![("files", files)],
        })
    }
}
