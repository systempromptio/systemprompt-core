//! Shared helpers for the sync integration suite: DB acquisition (with
//! environment-skip), tenant id factory, and tiny wiremock builders.

use systemprompt_database::DbPool;
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

pub async fn try_db() -> Option<DbPool> {
    let url = fixture_database_url().ok()?;
    Some(
        fixture_db_pool(&url)
            .await
            .expect("connect to test database"),
    )
}

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
