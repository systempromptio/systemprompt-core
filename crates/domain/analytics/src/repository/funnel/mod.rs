//! Conversion-funnel definitions and per-session progress tracking.
//!
//! [`FunnelRepository`] manages `funnels` and their ordered `funnel_steps`,
//! records `funnel_progress` as sessions advance, and computes drop-off
//! statistics. Mutations live in `mutations`, reads in `finders` and `stats`,
//! and shared row types in `types`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod finders;
mod mutations;
mod stats;
mod types;

use crate::Result;
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;

#[derive(Clone, Debug)]
pub struct FunnelRepository {
    pool: Arc<PgPool>,
    write_pool: Arc<PgPool>,
}

impl FunnelRepository {
    pub fn new(db: &DbPool) -> Result<Self> {
        let pool = db.pool_arc()?;
        let write_pool = db.write_pool_arc()?;
        Ok(Self { pool, write_pool })
    }
}
