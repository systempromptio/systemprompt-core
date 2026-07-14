//! DB-backed `SyncService::sync_database` / `sync_all` runs: pull round-trip
//! through the export/import upserts, dry-run reporting, and the partial-
//! import failure mapping in `sync_all`.

use serde_json::json;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{SessionId, TenantId, UserId};
use systemprompt_sync::{SyncConfig, SyncDirection, SyncOpState, SyncService};
use systemprompt_test_fixtures::{
    fixture_database_url, fixture_db_pool, seed_user_row, seed_user_session, unique_user_id,
};
use tempfile::TempDir;
use uuid::Uuid;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

macro_rules! db_or_skip {
    () => {{
        let Ok(url) = fixture_database_url() else {
            return;
        };
        let Ok(pool) = fixture_db_pool(&url).await else {
            return;
        };
        (url, pool)
    }};
}

async fn mount_database_url(server: &MockServer, tenant: &str, database_url: &str) {
    Mock::given(method("GET"))
        .and(path(format!("/api/v1/cloud/tenants/{tenant}/database")))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({ "database_url": database_url })),
        )
        .mount(server)
        .await;
}

fn config(
    tenant: &str,
    api_url: &str,
    services_path: &str,
    local_db: &str,
    direction: SyncDirection,
    dry_run: bool,
) -> SyncConfig {
    SyncConfig::builder(TenantId::new(tenant), api_url, "tok", services_path)
        .with_direction(direction)
        .with_dry_run(dry_run)
        .with_local_database_url(local_db)
        .build()
}

async fn seed_user_and_context(pool: &DbPool, tag: &str) -> (UserId, String) {
    let user_id = unique_user_id(tag);
    let email = format!("{}@sync.example.com", user_id.as_str());
    seed_user_row(pool, &user_id, &email).await.expect("user");
    let session_id = SessionId::generate();
    seed_user_session(pool, &user_id, &session_id)
        .await
        .expect("session");

    let context_id = format!("ctx-{}", Uuid::new_v4());
    let p = pool.pool_arc().expect("pool");
    sqlx::query!(
        "INSERT INTO user_contexts (context_id, user_id, session_id, name) VALUES ($1, $2, $3, \
         $4)",
        context_id,
        user_id.as_str(),
        session_id.as_str(),
        "sync-roundtrip"
    )
    .execute(p.as_ref())
    .await
    .expect("context");

    (user_id, context_id)
}

#[tokio::test]
async fn sync_database_pull_roundtrips_users_and_contexts() {
    let (url, pool) = db_or_skip!();
    let (user_id, context_id) = seed_user_and_context(&pool, "sync-pull").await;

    let server = MockServer::start().await;
    mount_database_url(&server, "t-db-pull", &url).await;

    let service = SyncService::new(config(
        "t-db-pull",
        &server.uri(),
        "/services",
        &url,
        SyncDirection::Pull,
        false,
    ))
    .expect("service");

    let result = service.sync_database().await.expect("sync");
    assert!(result.success);
    assert_eq!(result.operation, "database_pull");
    assert!(result.items_synced >= 2, "{}", result.items_synced);
    assert_eq!(result.state, SyncOpState::Completed);

    let p = pool.pool_arc().expect("pool");
    let name: String = sqlx::query_scalar!(
        "SELECT name FROM user_contexts WHERE context_id = $1",
        context_id
    )
    .fetch_one(p.as_ref())
    .await
    .expect("context survives roundtrip");
    assert_eq!(name, "sync-roundtrip");

    let email: String =
        sqlx::query_scalar!("SELECT email FROM users WHERE id = $1", user_id.as_str())
            .fetch_one(p.as_ref())
            .await
            .expect("user survives roundtrip");
    assert!(email.ends_with("@sync.example.com"));
}

#[tokio::test]
async fn sync_database_push_dry_run_counts_without_importing() {
    let (url, pool) = db_or_skip!();
    seed_user_and_context(&pool, "sync-dry").await;

    let server = MockServer::start().await;
    mount_database_url(&server, "t-db-dry", &url).await;

    let service = SyncService::new(config(
        "t-db-dry",
        &server.uri(),
        "/services",
        &url,
        SyncDirection::Push,
        true,
    ))
    .expect("service");

    let result = service.sync_database().await.expect("sync");
    assert!(result.success);
    assert_eq!(result.operation, "database_push");
    assert_eq!(result.items_synced, 0);
    assert!(result.items_skipped >= 2);
    let details = result.details.expect("details");
    assert!(details["users"].as_u64().expect("users") >= 1);
    assert!(details["contexts"].as_u64().expect("contexts") >= 1);
}

#[tokio::test]
async fn sync_database_pull_dry_run_counts_without_importing() {
    let (url, pool) = db_or_skip!();
    seed_user_and_context(&pool, "sync-dry-pull").await;

    let server = MockServer::start().await;
    mount_database_url(&server, "t-db-dry-pull", &url).await;

    let service = SyncService::new(config(
        "t-db-dry-pull",
        &server.uri(),
        "/services",
        &url,
        SyncDirection::Pull,
        true,
    ))
    .expect("service");

    let result = service.sync_database().await.expect("sync");
    assert!(result.success);
    assert_eq!(result.operation, "database_pull");
    assert!(result.items_skipped >= 2);
}

#[tokio::test]
async fn sync_all_reports_partial_state_when_import_target_lacks_schema() {
    let (url, pool) = db_or_skip!();
    seed_user_and_context(&pool, "sync-partial").await;

    let maintenance_url = {
        let idx = url.rfind('/').expect("db name separator");
        format!("{}/postgres", &url[..idx])
    };
    if sqlx::PgPool::connect(&maintenance_url).await.is_err() {
        return;
    }

    let services = TempDir::new().expect("tempdir");
    let server = MockServer::start().await;
    mount_database_url(&server, "t-db-partial", &maintenance_url).await;
    Mock::given(method("POST"))
        .and(path("/api/v1/cloud/tenants/t-db-partial/files"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "files_uploaded": 0 })))
        .mount(&server)
        .await;

    let service = SyncService::new(config(
        "t-db-partial",
        &server.uri(),
        &services.path().to_string_lossy(),
        &url,
        SyncDirection::Push,
        false,
    ))
    .expect("service");

    let results = service.sync_all().await.expect("sync_all");
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].operation, "files_push");
    assert!(results[0].success);

    let db_result = &results[1];
    assert_eq!(db_result.operation, "database");
    assert!(!db_result.success);
    assert!(matches!(
        db_result.state,
        SyncOpState::Partial { completed: 0, .. }
    ));
    assert_eq!(db_result.items_synced, 0);
    assert!(
        db_result.errors[0].contains("Partial import failure"),
        "{:?}",
        db_result.errors
    );
}

#[tokio::test]
async fn sync_all_succeeds_end_to_end_against_same_database() {
    let (url, pool) = db_or_skip!();
    seed_user_and_context(&pool, "sync-all-ok").await;

    let services = TempDir::new().expect("tempdir");
    let server = MockServer::start().await;
    mount_database_url(&server, "t-db-ok", &url).await;
    Mock::given(method("POST"))
        .and(path("/api/v1/cloud/tenants/t-db-ok/files"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "files_uploaded": 0 })))
        .mount(&server)
        .await;

    let service = SyncService::new(config(
        "t-db-ok",
        &server.uri(),
        &services.path().to_string_lossy(),
        &url,
        SyncDirection::Push,
        false,
    ))
    .expect("service");

    let results = service.sync_all().await.expect("sync_all");
    assert_eq!(results.len(), 2);
    assert!(results[0].success);
    assert!(results[1].success);
    assert_eq!(results[1].operation, "database_push");
    assert!(results[1].items_synced >= 2);
}
