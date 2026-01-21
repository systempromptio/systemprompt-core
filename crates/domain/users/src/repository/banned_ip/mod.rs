mod listing;
mod queries;
mod types;

use std::sync::Arc;

use anyhow::Result;
use sqlx::PgPool;
use systemprompt_database::DbPool;

pub use types::{BanDuration, BanIpParams, BanIpWithMetadataParams, BannedIp};

#[derive(Clone, Debug)]
pub struct BannedIpRepository {
    pool: Arc<PgPool>,
}

impl BannedIpRepository {
    pub fn new(db: &DbPool) -> Result<Self> {
        let pool = db.pool_arc()?;
        Ok(Self { pool })
    }

    pub const fn from_pool(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}
