// DB-backed bridge-session repository tests (upsert, list-active, delete-stale).

use std::time::Duration;

use systemprompt_identifiers::{SessionId, UserId};
use systemprompt_oauth::repository::{BridgeSessionRepository, UpsertBridgeSession};
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_database_url, fixture_db_pool, seed_user_row, unique_user_id,
};

struct Ctx {
    repo: BridgeSessionRepository,
    user_id: UserId,
}

async fn setup() -> Option<Ctx> {
    let url = fixture_database_url().ok()?;
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = BridgeSessionRepository::new(&pool).expect("repo");
    let user_id = unique_user_id("bs");
    seed_user_row(&pool, &user_id, &format!("{}@bs.invalid", user_id.as_str()))
        .await
        .expect("seed user");
    Some(Ctx { repo, user_id })
}

fn upsert_params(session_id: &SessionId, user_id: &UserId) -> UpsertBridgeSession {
    UpsertBridgeSession {
        session_id: session_id.clone(),
        user_id: user_id.clone(),
        bridge_version: "1.0.0".to_owned(),
        os: "linux".to_owned(),
        hostname: "host-a".to_owned(),
        last_activity_at: None,
        forwarded_total: 3,
        tokens_in_total: 100,
        tokens_out_total: 200,
    }
}

#[tokio::test]
async fn upsert_then_list_active() {
    let Some(ctx) = setup().await else { return };
    let session_id = SessionId::generate();
    ctx.repo
        .upsert(upsert_params(&session_id, &ctx.user_id))
        .await
        .expect("upsert");

    let rows = ctx
        .repo
        .list_active(Duration::from_secs(3600))
        .await
        .expect("list_active");
    let found = rows
        .iter()
        .find(|r| r.session_id == session_id)
        .expect("present");
    assert_eq!(found.user_id, ctx.user_id);
    assert_eq!(found.bridge_version, "1.0.0");
    assert_eq!(found.forwarded_total, 3);
    assert_eq!(found.tokens_in_total, 100);
    assert_eq!(found.tokens_out_total, 200);
}

#[tokio::test]
async fn upsert_updates_existing_row() {
    let Some(ctx) = setup().await else { return };
    let session_id = SessionId::generate();
    ctx.repo
        .upsert(upsert_params(&session_id, &ctx.user_id))
        .await
        .expect("upsert");

    let mut updated = upsert_params(&session_id, &ctx.user_id);
    updated.bridge_version = "2.0.0".to_owned();
    updated.forwarded_total = 9;
    ctx.repo.upsert(updated).await.expect("upsert again");

    let rows = ctx
        .repo
        .list_active_for_user(&ctx.user_id, Duration::from_secs(3600))
        .await
        .expect("list for user");
    let found = rows
        .iter()
        .find(|r| r.session_id == session_id)
        .expect("present");
    assert_eq!(found.bridge_version, "2.0.0");
    assert_eq!(found.forwarded_total, 9);
}

#[tokio::test]
async fn list_active_excludes_old_heartbeats() {
    let Some(ctx) = setup().await else { return };
    let session_id = SessionId::generate();
    ctx.repo
        .upsert(upsert_params(&session_id, &ctx.user_id))
        .await
        .expect("upsert");

    // A zero-second window excludes the just-written heartbeat.
    let rows = ctx
        .repo
        .list_active(Duration::from_secs(0))
        .await
        .expect("list_active");
    assert!(!rows.iter().any(|r| r.session_id == session_id));
}

#[tokio::test]
async fn delete_stale_removes_recent_with_zero_window() {
    let Some(ctx) = setup().await else { return };
    let session_id = SessionId::generate();
    ctx.repo
        .upsert(upsert_params(&session_id, &ctx.user_id))
        .await
        .expect("upsert");

    // older_than = 0 → cutoff is "now", so the just-written row is stale.
    let removed = ctx
        .repo
        .delete_stale(Duration::from_secs(0))
        .await
        .expect("delete_stale");
    assert!(removed >= 1);
}
