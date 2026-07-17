//! Shared pool alias for repository implementations.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgPool;
use std::sync::Arc;

pub type PgDbPool = Arc<PgPool>;
