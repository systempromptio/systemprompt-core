//! Shared pool alias for repository implementations.

use sqlx::PgPool;
use std::sync::Arc;

pub type PgDbPool = Arc<PgPool>;
