mod banned_ip;
mod user;

pub use banned_ip::{
    BanDuration, BanIpParams, BanIpWithMetadataParams, BannedIp, BannedIpRepository,
};
pub use user::{MergeResult, UpdateUserParams};

use anyhow::Result;
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;

pub(crate) const MAX_PAGE_SIZE: i64 = 100;

#[derive(Debug, Clone)]
pub struct UserRepository {
    pool: Arc<PgPool>,
}

impl UserRepository {
    pub fn new(db: &DbPool) -> Result<Self> {
        let pool = db.pool_arc()?;
        Ok(Self { pool })
    }
}
