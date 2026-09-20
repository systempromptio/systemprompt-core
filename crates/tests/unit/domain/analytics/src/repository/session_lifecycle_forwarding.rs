#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::sync::Arc;

use chrono::{Duration, Utc};
use sqlx::Row;
use systemprompt_identifiers::UserId;
use systemprompt_test_fixtures::{
    DisposableDb, ensure_test_bootstrap, fixture_database_url, fixture_db_pool, seed_user_row,
};
use uuid::Uuid;

use super::session_support::{base_params, delete_session, unique_session_id};

async fn repository() -> (
    systemprompt_database::DbPool,
    systemprompt_analytics::SessionRepository,
) {
    let url = fixture_database_url().expect("analytics fixture database URL");
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("analytics fixture pool");
    let repo = systemprompt_test_fixtures::fixture_analytics_repositories(&pool)
        .expect("analytics repositories")
        .sessions;
    (pool, repo)
}

async fn isolated_repository(
    prefix: &str,
) -> (
    DisposableDb,
    systemprompt_database::DbPool,
    systemprompt_analytics::SessionRepository,
) {
    let database = DisposableDb::installed(prefix).await.unwrap();
    let pool = database.pool().await.unwrap();
    let repo = systemprompt_test_fixtures::fixture_analytics_repositories(&pool)
        .unwrap()
        .sessions;
    (database, pool, repo)
}

async fn drop_isolated(database: DisposableDb, pool: systemprompt_database::DbPool) {
    pool.write_pool_arc().unwrap().close().await;
    database.drop_now().await;
}

async fn user(pool: &systemprompt_database::DbPool, prefix: &str) -> UserId {
    let id = UserId::new(format!("{prefix}-{}", Uuid::new_v4()));
    seed_user_row(pool, &id, &format!("{}@sessions.invalid", id.as_str()))
        .await
        .unwrap();
    id
}

#[tokio::test]
async fn create_and_owner_scoped_lookups_preserve_identity_and_fingerprint() {
    let (pool, repo) = repository().await;
    let owner = user(&pool, "analytics-owner").await;
    let other = user(&pool, "analytics-other").await;
    let sid = unique_session_id();
    let other_sid = unique_session_id();
    let fingerprint = format!("owner-fp-{}", Uuid::new_v4());
    let other_fingerprint = format!("other-fp-{}", Uuid::new_v4());

    let mut params = base_params(&sid, Some(&fingerprint), Utc::now() + Duration::hours(1));
    params.user_id = Some(&owner);
    params.ip_address = Some("203.0.113.9");
    params.user_agent = Some("lifecycle-agent");
    repo.create_session(&params).await.unwrap();
    let mut other_params = base_params(
        &other_sid,
        Some(&other_fingerprint),
        Utc::now() + Duration::hours(1),
    );
    other_params.user_id = Some(&other);
    repo.create_session(&other_params).await.unwrap();

    let found = repo.find_by_id(&sid).await.unwrap().unwrap();
    assert_eq!(found.user_id.as_ref(), Some(&owner));
    assert_eq!(
        found.fingerprint_hash.as_deref(),
        Some(fingerprint.as_str())
    );
    let by_fingerprint = repo
        .find_by_fingerprint(&fingerprint, &owner)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(by_fingerprint.session_id, sid);
    assert!(
        repo.find_by_fingerprint(&fingerprint, &other)
            .await
            .unwrap()
            .is_none()
    );

    let active = repo.list_active_by_user(&owner).await.unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].session_id, sid);
    let recent = repo
        .find_recent_by_fingerprint(&fingerprint, 3600)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(recent.session_id, sid);
    assert_eq!(recent.user_id.as_ref(), Some(&owner));

    delete_session(&pool, &sid).await;
    delete_session(&pool, &other_sid).await;
}

#[tokio::test]
async fn activity_usage_scanner_and_conversion_mutations_persist_together() {
    let (pool, repo) = repository().await;
    let sid = unique_session_id();
    let fingerprint = format!("usage-fp-{}", Uuid::new_v4());
    repo.create_session(&base_params(
        &sid,
        Some(&fingerprint),
        Utc::now() + Duration::hours(1),
    ))
    .await
    .unwrap();
    let db = pool.pool_arc().unwrap();
    sqlx::query("UPDATE user_sessions SET last_activity_at = CURRENT_TIMESTAMP - INTERVAL '2 hours' WHERE session_id = $1")
        .bind(sid.as_str())
        .execute(db.as_ref())
        .await
        .unwrap();
    let before: chrono::DateTime<Utc> =
        sqlx::query("SELECT last_activity_at FROM user_sessions WHERE session_id = $1")
            .bind(sid.as_str())
            .fetch_one(db.as_ref())
            .await
            .unwrap()
            .get("last_activity_at");

    repo.update_activity(&sid).await.unwrap();
    repo.increment_request_count(&sid).await.unwrap();
    repo.increment_task_count(&sid).await.unwrap();
    repo.increment_task_count(&sid).await.unwrap();
    repo.increment_message_count(&sid).await.unwrap();
    repo.increment_ai_usage(&sid, 120, 7_500).await.unwrap();
    repo.increment_ai_usage(&sid, 30, 2_500).await.unwrap();
    repo.mark_as_scanner(&sid).await.unwrap();
    repo.mark_converted(&sid).await.unwrap();

    let snapshot = repo.find_by_id(&sid).await.unwrap().unwrap();
    assert_eq!(snapshot.request_count, Some(1));
    assert_eq!(snapshot.task_count, Some(2));
    assert_eq!(snapshot.message_count, Some(1));
    assert_eq!(snapshot.ai_request_count, Some(2));
    assert_eq!(snapshot.is_scanner, Some(true));
    let row = sqlx::query("SELECT last_activity_at, total_tokens_used, total_ai_cost_microdollars, converted_at FROM user_sessions WHERE session_id = $1")
        .bind(sid.as_str())
        .fetch_one(db.as_ref())
        .await
        .unwrap();
    assert!(row.get::<chrono::DateTime<Utc>, _>("last_activity_at") > before);
    assert_eq!(row.get::<i32, _>("total_tokens_used"), 150);
    assert_eq!(row.get::<i64, _>("total_ai_cost_microdollars"), 10_000);
    assert!(
        row.get::<Option<chrono::DateTime<Utc>>, _>("converted_at")
            .is_some()
    );

    delete_session(&pool, &sid).await;
}

#[tokio::test]
async fn migration_moves_only_source_owner_then_bulk_revoke_removes_active_access() {
    let (pool, repo) = repository().await;
    let old = user(&pool, "migration-old").await;
    let new = user(&pool, "migration-new").await;
    let unrelated = user(&pool, "migration-unrelated").await;
    let first = unique_session_id();
    let second = unique_session_id();
    let outsider = unique_session_id();
    for (sid, owner, fingerprint) in [
        (&first, &old, "migration-first"),
        (&second, &old, "migration-second"),
        (&outsider, &unrelated, "migration-outsider"),
    ] {
        let mut params = base_params(sid, Some(fingerprint), Utc::now() + Duration::hours(1));
        params.user_id = Some(owner);
        repo.create_session(&params).await.unwrap();
    }

    assert_eq!(repo.migrate_user_sessions(&old, &new).await.unwrap(), 2);
    assert!(repo.list_active_by_user(&old).await.unwrap().is_empty());
    let migrated = repo.list_active_by_user(&new).await.unwrap();
    assert_eq!(migrated.len(), 2);
    assert!(
        migrated
            .iter()
            .all(|session| session.user_id.as_ref() == Some(&new))
    );
    assert_eq!(repo.list_active_by_user(&unrelated).await.unwrap().len(), 1);

    assert_eq!(repo.revoke_all_for_user(&new).await.unwrap(), 2);
    assert!(repo.find_active_by_id(&first).await.unwrap().is_none());
    assert!(repo.find_active_by_id(&outsider).await.unwrap().is_some());
    let db = pool.pool_arc().unwrap();
    let revoked = sqlx::query_scalar::<_, bool>(
        "SELECT bool_and(revoked_at IS NOT NULL) FROM user_sessions WHERE user_id = $1",
    )
    .bind(new.as_str())
    .fetch_one(db.as_ref())
    .await
    .unwrap();
    assert!(revoked, "every migrated session was durably revoked");

    delete_session(&pool, &first).await;
    delete_session(&pool, &second).await;
    delete_session(&pool, &outsider).await;
}

#[tokio::test]
async fn inactive_cleanup_ends_only_stale_sessions_and_preserves_fresh_session() {
    let (database, pool, repo) = isolated_repository("analytics_session_cleanup").await;
    let stale = unique_session_id();
    let fresh = unique_session_id();
    for sid in [&stale, &fresh] {
        let fingerprint = format!("cleanup-{}", sid.as_str());
        repo.create_session(&base_params(
            sid,
            Some(&fingerprint),
            Utc::now() + Duration::hours(1),
        ))
        .await
        .unwrap();
    }
    let db = pool.pool_arc().unwrap();
    sqlx::query("UPDATE user_sessions SET last_activity_at = CURRENT_TIMESTAMP - INTERVAL '8 hours' WHERE session_id = $1")
        .bind(stale.as_str())
        .execute(db.as_ref())
        .await
        .unwrap();

    let before = repo.count_inactive(2).await.unwrap();
    assert_eq!(before, 1);
    assert_eq!(repo.cleanup_inactive(2).await.unwrap(), 1);
    assert!(
        repo.find_by_id(&stale)
            .await
            .unwrap()
            .unwrap()
            .ended_at
            .is_some()
    );
    assert!(
        repo.find_by_id(&fresh)
            .await
            .unwrap()
            .unwrap()
            .ended_at
            .is_none(),
        "cleanup must preserve the fresh session"
    );

    delete_session(&pool, &stale).await;
    delete_session(&pool, &fresh).await;
    drop_isolated(database, pool).await;
}

#[tokio::test]
async fn geo_backfill_enriches_public_ip_across_single_row_pages_and_skips_private_ip() {
    let (database, pool, repo) = isolated_repository("analytics_session_geo").await;
    let public = unique_session_id();
    let private = unique_session_id();
    for (sid, ip) in [(&public, "89.160.20.128"), (&private, "127.0.0.1")] {
        let fingerprint = format!("geo-{}", sid.as_str());
        let mut params = base_params(sid, Some(&fingerprint), Utc::now() + Duration::hours(1));
        params.ip_address = Some(ip);
        repo.create_session(&params).await.unwrap();
    }
    let reader = Arc::new(
        maxminddb::Reader::open_readfile(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/fixtures/GeoIP2-City-Test.mmdb"
        ))
        .unwrap(),
    );

    assert_eq!(
        repo.backfill_session_geo(Some(&reader), 1).await.unwrap(),
        1
    );
    let enriched = repo.find_by_id(&public).await.unwrap().unwrap();
    assert_eq!(enriched.country.as_deref(), Some("SE"));
    assert_eq!(enriched.city.as_deref(), Some("Linköping"));
    let skipped = repo.find_by_id(&private).await.unwrap().unwrap();
    assert!(skipped.country.is_none());
    assert!(repo.count_sessions_missing_geo().await.unwrap() >= 1);

    delete_session(&pool, &public).await;
    delete_session(&pool, &private).await;
    drop_isolated(database, pool).await;
}
