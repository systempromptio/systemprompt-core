//! Behavioural tests for the file-fed ingestion entry point
//! (`ingest_config_from_yaml_path`) and the `delete_orphans` option.
//!
//! Each test scopes itself to a unique entity id and cleans up on the way
//! out, and skips cleanly when no fixture database is reachable.

use std::path::PathBuf;

use systemprompt_database::DbPool;
use systemprompt_security::authz::{
    AccessControlIngestionService, IngestOptions, RegisteredEntities,
};
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};
use uuid::Uuid;

async fn pool() -> Option<DbPool> {
    let url = fixture_database_url().ok()?;
    fixture_db_pool(&url).await.ok()
}

fn unique_id(prefix: &str) -> String {
    format!("{prefix}-{}", Uuid::new_v4().simple())
}

fn write_temp_yaml(contents: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("authz-ingest-{}.yaml", Uuid::new_v4().simple()));
    std::fs::write(&path, contents).expect("temp yaml written");
    path
}

async fn cleanup(db: &DbPool, entity_type: &str, entity_id: &str) {
    let pg = db.write_pool_arc().expect("write pool");
    sqlx::query("DELETE FROM access_control_rules WHERE entity_type = $1 AND entity_id = $2")
        .bind(entity_type)
        .bind(entity_id)
        .execute(&*pg)
        .await
        .expect("cleanup rules");
    sqlx::query("DELETE FROM access_control_entities WHERE entity_type = $1 AND entity_id = $2")
        .bind(entity_type)
        .bind(entity_id)
        .execute(&*pg)
        .await
        .expect("cleanup entities");
}

async fn role_rule_count(db: &DbPool, entity_id: &str) -> i64 {
    let pg = db.write_pool_arc().expect("write pool");
    sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM access_control_rules WHERE entity_id = $1 AND rule_type = 'role'",
    )
    .bind(entity_id)
    .fetch_one(&*pg)
    .await
    .expect("count role rules")
}

#[tokio::test]
async fn a_yaml_file_on_disk_is_ingested_the_same_as_a_parsed_config() {
    let Some(db) = pool().await else {
        return;
    };
    let svc = AccessControlIngestionService::new(&db).expect("ingestion service");
    let id = unique_id("ing-path");
    let path = write_temp_yaml(&format!(
        "rules:\n  - entity_type: gateway_route\n    entity_id: {id}\n    access: allow\n    \
         roles: [from-file]\n"
    ));

    let report = svc
        .ingest_config_from_yaml_path(
            &path,
            IngestOptions::default(),
            &RegisteredEntities::default(),
        )
        .await
        .expect("file ingest");

    assert_eq!(report.inserted, 1);
    assert_eq!(role_rule_count(&db, &id).await, 1);

    std::fs::remove_file(&path).ok();
    cleanup(&db, "gateway_route", &id).await;
}

#[tokio::test]
async fn a_missing_yaml_file_is_a_validation_error_naming_the_path() {
    let Some(db) = pool().await else {
        return;
    };
    let svc = AccessControlIngestionService::new(&db).expect("ingestion service");
    let path = std::env::temp_dir().join(format!("absent-{}.yaml", Uuid::new_v4().simple()));

    let err = svc
        .ingest_config_from_yaml_path(
            &path,
            IngestOptions::default(),
            &RegisteredEntities::default(),
        )
        .await
        .expect_err("a missing file must not silently ingest nothing");

    let message = err.to_string();
    assert!(
        message.contains(&path.display().to_string()),
        "the error must name the unreadable path, got {message}"
    );
}

#[tokio::test]
async fn a_yaml_file_that_is_not_an_access_control_config_is_rejected() {
    let Some(db) = pool().await else {
        return;
    };
    let svc = AccessControlIngestionService::new(&db).expect("ingestion service");
    let path = write_temp_yaml("rules: \"this is not a sequence\"\n");

    let err = svc
        .ingest_config_from_yaml_path(
            &path,
            IngestOptions::default(),
            &RegisteredEntities::default(),
        )
        .await
        .expect_err("a malformed document must not ingest");

    let message = err.to_string();
    assert!(
        message.contains("AccessControlConfig"),
        "the error must say the document did not parse as a config, got {message}"
    );

    std::fs::remove_file(&path).ok();
}

#[tokio::test]
async fn delete_orphans_clears_stale_role_grants_before_reapplying_the_config() {
    let Some(db) = pool().await else {
        return;
    };
    let svc = AccessControlIngestionService::new(&db).expect("ingestion service");
    let id = unique_id("ing-orphan");

    let two_roles = format!(
        "rules:\n  - entity_type: gateway_route\n    entity_id: {id}\n    access: allow\n    \
         roles: [alpha, beta]\n"
    );
    svc.ingest_config(
        &serde_yaml::from_str(&two_roles).expect("yaml"),
        IngestOptions::default(),
        &RegisteredEntities::default(),
    )
    .await
    .expect("seed two role grants");
    assert_eq!(role_rule_count(&db, &id).await, 2);

    let one_role = format!(
        "rules:\n  - entity_type: gateway_route\n    entity_id: {id}\n    access: allow\n    \
         roles: [alpha]\n"
    );
    let report = svc
        .ingest_config(
            &serde_yaml::from_str(&one_role).expect("yaml"),
            IngestOptions {
                override_existing: true,
                delete_orphans: true,
            },
            &RegisteredEntities::default(),
        )
        .await
        .expect("reingest with orphan deletion");

    assert!(
        report.deleted >= 2,
        "the pre-existing role grants must be swept, got {report:?}"
    );
    assert_eq!(
        role_rule_count(&db, &id).await,
        1,
        "only the role still named by the config may survive"
    );

    cleanup(&db, "gateway_route", &id).await;
}
