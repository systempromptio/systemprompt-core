//! Shared helpers for the sync integration suite: DB acquisition (with
//! environment-skip), tenant id factory, and tiny wiremock builders.

use systemprompt_database::DbPool;
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

/// Returns a `DbPool` against `DATABASE_URL`, or `None` if the variable
/// is unset so the test can early-skip without failing.
pub async fn try_db() -> Option<DbPool> {
    let url = fixture_database_url().ok()?;
    Some(
        fixture_db_pool(&url)
            .await
            .expect("connect to test database"),
    )
}

/// Wipe the rows the sync integration tests touch under the given
/// (entity_type, entity_id) pair. ACL rules are tenant-agnostic — callers
/// must pass entity ids that are unique to the test.
pub async fn wipe_rules(db: &DbPool, entity_type: &str, entity_id: &str) {
    let pool = db.write_pool_arc().expect("write pool");
    sqlx::query!(
        "DELETE FROM access_control_rules WHERE entity_type = $1 AND entity_id = $2",
        entity_type,
        entity_id
    )
    .execute(&*pool)
    .await
    .expect("wipe access_control_rules");
}
