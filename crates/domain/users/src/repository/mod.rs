mod api_key;
mod banned_ip;
mod device_cert;
mod user;

pub use api_key::CreateApiKeyParams;
pub use banned_ip::{
    BanDuration, BanIpParams, BanIpWithMetadataParams, BannedIp, BannedIpRepository,
};
pub use device_cert::EnrollDeviceCertParams;
pub use user::{MergeResult, UpdateUserParams};

use anyhow::Result;
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;

pub const MAX_PAGE_SIZE: i64 = 100;

#[derive(Debug, Clone)]
pub struct UserRepository {
    pool: Arc<PgPool>,
    write_pool: Arc<PgPool>,
}

impl UserRepository {
    pub fn new(db: &DbPool) -> Result<Self> {
        let pool = db.pool_arc()?;
        let write_pool = db.write_pool_arc()?;
        Ok(Self { pool, write_pool })
    }
}
