//! DB-backed tests for the pool-seamed `admin access-control export-yaml`
//! command, driving `render_yaml_snapshot` directly against a fixture pool.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::admin::access_control::export::render_yaml_snapshot;
use systemprompt_database::DbPool;
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};
use uuid::Uuid;

async fn pool() -> DbPool {
    fixture_db_pool(&fixture_database_url().unwrap())
        .await
        .unwrap()
}

async fn seed_role_rule(pool: &DbPool, entity_id: &str, role: &str, justification: Option<&str>) {
    let raw = pool.pool_arc().unwrap();
    sqlx::query(
        "INSERT INTO access_control_entities (entity_type, entity_id, default_included, source) \
         VALUES ('agent', $1, false, 'test') ON CONFLICT DO NOTHING",
    )
    .bind(entity_id)
    .execute(raw.as_ref())
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO access_control_rules \
         (entity_type, entity_id, rule_type, rule_value, access, justification) \
         VALUES ('agent', $1, 'role', $2, 'allow', $3)",
    )
    .bind(entity_id)
    .bind(role)
    .bind(justification)
    .execute(raw.as_ref())
    .await
    .unwrap();
}

#[tokio::test]
async fn snapshot_includes_seeded_role_rule() {
    let pool = pool().await;
    let entity_id = format!("cov-agent-{}", Uuid::new_v4().simple());
    let role = format!("cov-role-{}", Uuid::new_v4().simple());
    seed_role_rule(&pool, &entity_id, &role, Some("granted for coverage")).await;

    let yaml = render_yaml_snapshot(&pool).await.unwrap();

    assert!(yaml.contains("rules:"));
    assert!(yaml.contains(&entity_id), "entity_id must appear in snapshot");
    assert!(yaml.contains(&role), "role must appear in snapshot");
    assert!(yaml.contains("granted for coverage"));
    assert!(yaml.contains("entity_type: agent"));
}

#[tokio::test]
async fn snapshot_omits_user_rules() {
    let pool = pool().await;
    let entity_id = format!("cov-agent-{}", Uuid::new_v4().simple());
    let user_val = format!("cov-user-{}", Uuid::new_v4().simple());

    let raw = pool.pool_arc().unwrap();
    sqlx::query(
        "INSERT INTO access_control_entities (entity_type, entity_id, default_included, source) \
         VALUES ('agent', $1, false, 'test') ON CONFLICT DO NOTHING",
    )
    .bind(&entity_id)
    .execute(raw.as_ref())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO access_control_rules \
         (entity_type, entity_id, rule_type, rule_value, access, justification) \
         VALUES ('agent', $1, 'user', $2, 'allow', NULL)",
    )
    .bind(&entity_id)
    .bind(&user_val)
    .execute(raw.as_ref())
    .await
    .unwrap();

    let yaml = render_yaml_snapshot(&pool).await.unwrap();

    assert!(
        !yaml.contains(&user_val),
        "user-type rule value must not appear in the role snapshot"
    );
}
