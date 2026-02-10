use sqlx::PgPool;
use std::sync::Arc;

pub type PgDbPool = Arc<PgPool>;
