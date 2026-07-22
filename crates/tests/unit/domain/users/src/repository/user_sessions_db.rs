//! DB-backed tests for user-session repository queries.

use systemprompt_identifiers::{SessionId, UserId};
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_database_url, fixture_db_pool, seed_user_row, seed_user_session,
};
use systemprompt_users::UserService;
use uuid::Uuid;

struct Ctx {
    service: UserService,
    user_id: UserId,
}

async fn setup(prefix: &str) -> Option<Ctx> {
    let url = fixture_database_url().ok()?;
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let user_id = UserId::new(Uuid::new_v4().to_string());
    let email = format!("{prefix}-{}@sess.invalid", Uuid::new_v4().simple());
    seed_user_row(&pool, &user_id, &email).await.expect("user");
    let service = UserService::new(&pool).expect("service");
    Some(Ctx { service, user_id })
}

async fn seed_session(ctx: &Ctx, url: &str) -> SessionId {
    let pool = fixture_db_pool(url).await.expect("pool");
    let session_id = SessionId::generate();
    seed_user_session(&pool, &ctx.user_id, &session_id)
        .await
        .expect("session");
    session_id
}

#[tokio::test]
async fn list_sessions_returns_all_active_and_ended() {
    let Some(ctx) = setup("list").await else {
        return;
    };
    let url = fixture_database_url().expect("url");
    let s1 = seed_session(&ctx, &url).await;
    let s2 = seed_session(&ctx, &url).await;

    let ended = ctx.service.end_session(&s1).await.expect("end");
    assert!(ended);

    let all = ctx.service.list_sessions(&ctx.user_id).await.expect("list");
    assert_eq!(all.len(), 2);
    assert!(
        all.iter()
            .any(|s| s.session_id == s1 && s.ended_at.is_some())
    );
    assert!(
        all.iter()
            .any(|s| s.session_id == s2 && s.ended_at.is_none())
    );

    let active = ctx
        .service
        .list_active_sessions(&ctx.user_id)
        .await
        .expect("active");
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].session_id, s2);
}

#[tokio::test]
async fn list_recent_sessions_applies_limit_and_clamps_oversized() {
    let Some(ctx) = setup("recent").await else {
        return;
    };
    let url = fixture_database_url().expect("url");
    for _ in 0..3 {
        seed_session(&ctx, &url).await;
    }

    let one = ctx
        .service
        .list_recent_sessions(&ctx.user_id, 1)
        .await
        .expect("limit 1");
    assert_eq!(one.len(), 1);

    let clamped = ctx
        .service
        .list_recent_sessions(&ctx.user_id, 10_000)
        .await
        .expect("clamped");
    assert_eq!(clamped.len(), 3);
}

#[tokio::test]
async fn end_session_is_idempotent_and_flips_existence() {
    let Some(ctx) = setup("end").await else {
        return;
    };
    let url = fixture_database_url().expect("url");
    let sid = seed_session(&ctx, &url).await;

    assert!(ctx.service.session_exists(&sid).await.expect("exists"));
    assert!(ctx.service.end_session(&sid).await.expect("first end"));
    assert!(!ctx.service.session_exists(&sid).await.expect("gone"));
    assert!(!ctx.service.end_session(&sid).await.expect("second end"));
}

#[tokio::test]
async fn session_exists_false_for_unknown_session() {
    let Some(ctx) = setup("unknown").await else {
        return;
    };
    let missing = SessionId::generate();
    assert!(!ctx.service.session_exists(&missing).await.expect("exists"));
}

#[tokio::test]
async fn end_all_sessions_ends_only_open_sessions() {
    let Some(ctx) = setup("endall").await else {
        return;
    };
    let url = fixture_database_url().expect("url");
    let s1 = seed_session(&ctx, &url).await;
    seed_session(&ctx, &url).await;
    assert!(ctx.service.end_session(&s1).await.expect("pre-end"));

    let ended = ctx
        .service
        .end_all_sessions(&ctx.user_id)
        .await
        .expect("end all");
    assert_eq!(ended, 1);

    let again = ctx
        .service
        .end_all_sessions(&ctx.user_id)
        .await
        .expect("second pass");
    assert_eq!(again, 0);
}
