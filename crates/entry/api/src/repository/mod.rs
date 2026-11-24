//! Composition helpers that construct single repositories for router state.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use systemprompt_database::DbPool;

pub fn config_plane_state(
    db: &DbPool,
) -> Arc<systemprompt_scheduler::repository::ConfigReconcileRepository> {
    Arc::new(systemprompt_scheduler::repository::ConfigReconcileRepository::new(db))
}
