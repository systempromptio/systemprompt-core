mod mutations;
mod queries;

use std::sync::Arc;

use anyhow::Result;
use sqlx::PgPool;
use systemprompt_database::DbPool;

pub const MAX_SESSIONS_PER_FINGERPRINT: i32 = 5;
pub const HIGH_REQUEST_THRESHOLD: i64 = 100;
pub const HIGH_VELOCITY_RPM: f32 = 10.0;
pub const SUSTAINED_VELOCITY_MINUTES: i32 = 60;
pub const ABUSE_THRESHOLD_FOR_BAN: i32 = 3;

#[derive(Clone, Debug)]
pub struct FingerprintRepository {
    pool: Arc<PgPool>,
}

impl FingerprintRepository {
    pub fn new(db: &DbPool) -> Result<Self> {
        let pool = db.pool_arc()?;
        Ok(Self { pool })
    }
}
