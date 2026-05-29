//! Persistence for IP bans.
//!
//! [`BannedIpRepository`] reads and writes the `banned_ips` table, supporting
//! temporary and permanent bans with offense metadata. Ban inputs are carried
//! by [`BanIpParams`] / [`BanIpWithMetadataParams`] with a [`BanDuration`], and
//! lookups return [`BannedIp`].

mod listing;
mod queries;
mod types;

use std::sync::Arc;

use crate::error::Result;
use sqlx::PgPool;
use systemprompt_database::DbPool;

pub use types::{BanDuration, BanIpParams, BanIpWithMetadataParams, BannedIp};

#[derive(Clone, Debug)]
pub struct BannedIpRepository {
    pool: Arc<PgPool>,
    write_pool: Arc<PgPool>,
}

impl BannedIpRepository {
    pub fn new(db: &DbPool) -> Result<Self> {
        let pool = db.pool_arc()?;
        let write_pool = db.write_pool_arc()?;
        Ok(Self { pool, write_pool })
    }

    pub fn from_pool(pool: Arc<PgPool>) -> Self {
        let write_pool = Arc::clone(&pool);
        Self { pool, write_pool }
    }
}
