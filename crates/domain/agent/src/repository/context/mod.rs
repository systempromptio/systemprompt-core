//! Repository for conversational contexts (multi-turn dialogue state).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod message;
mod mutations;
pub mod notifications;
mod queries;

pub use notifications::ContextNotificationRepository;

use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;

use crate::repository::task::TaskConstructor;

#[derive(Debug, Clone)]
pub struct ContextRepository {
    pool: Arc<PgPool>,
    write_pool: Arc<PgPool>,
    tasks: TaskConstructor,
}

impl ContextRepository {
    pub fn new(db: &DbPool) -> Self {
        let pool = db.pool();
        let write_pool = db.write_pool();
        Self {
            pool,
            write_pool,
            tasks: TaskConstructor::new(db),
        }
    }
}
