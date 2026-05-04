//! Shared pool alias for repository implementations.

use sqlx::PgPool;
use std::sync::Arc;

/// Shared `PostgreSQL` pool alias used as the constructor argument for every
/// repository in the workspace.
pub type PgDbPool = Arc<PgPool>;
