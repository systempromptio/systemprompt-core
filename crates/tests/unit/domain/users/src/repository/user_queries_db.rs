//! DB-backed tests for user lookup, listing, stats, bulk, merge, and cleanup
//! repository paths surfaced through `UserService`.

use systemprompt_identifiers::{SessionId, UserId};
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_database_url, fixture_db_pool, seed_user_session,
};
use systemprompt_users::{UserError, UserRole, UserService, UserStatus};
use uuid::Uuid;

struct Ctx {
    service: UserService,
    pool: systemprompt_database::DbPool,
}

async fn setup() -> Option<Ctx> {
    let url = fixture_database_url().ok()?;
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let service = UserService::new(&pool).expect("service");
    Some(Ctx { service, pool })
}

fn unique(prefix: &str) -> (String, String) {
    let tag = Uuid::new_v4().simple().to_string();
    (
        format!("{prefix}-{tag}"),
        format!("{prefix}-{tag}@q.invalid"),
    )
}

async fn create_user(ctx: &Ctx, prefix: &str) -> systemprompt_users::User {
    let (name, email) = unique(prefix);
    ctx.service
        .create(&name, &email, Some("Query Person"), None)
        .await
        .expect("create user")
}

async fn backdate_created_at(ctx: &Ctx, id: &UserId, days: i64) {
    let pg = ctx.pool.pool_arc().expect("pg pool");
    sqlx::query(
        "UPDATE users SET created_at = NOW() - make_interval(days => $1::int) WHERE id = $2",
    )
    .bind(days)
    .bind(id.as_str())
    .execute(pg.as_ref())
    .await
    .expect("backdate");
}

#[tokio::test]
async fn find_by_role_and_first_user_and_first_admin() {
    let Some(ctx) = setup().await else {
        return;
    };
    let user = create_user(&ctx, "role").await;
    let admin_roles = vec![
        UserRole::Admin.as_str().to_owned(),
        UserRole::User.as_str().to_owned(),
    ];
    ctx.service
        .assign_roles(&user.id, &admin_roles)
        .await
        .expect("assign admin");

    let admins = ctx
        .service
        .find_by_role(UserRole::Admin)
        .await
        .expect("by role");
    assert!(admins.iter().any(|u| u.id == user.id));

    assert!(
        ctx.service
            .find_first_user()
            .await
            .expect("first")
            .is_some()
    );
    assert!(
        ctx.service
            .find_first_admin()
            .await
            .expect("first admin")
            .is_some()
    );

    ctx.service.delete(&user.id).await.expect("cleanup");
}

#[tokio::test]
async fn find_authenticated_user_requires_active_status() {
    let Some(ctx) = setup().await else {
        return;
    };
    let user = create_user(&ctx, "authd").await;

    assert!(
        ctx.service
            .find_authenticated_user(&user.id)
            .await
            .expect("active lookup")
            .is_some()
    );

    ctx.service
        .update_status(&user.id, UserStatus::Suspended)
        .await
        .expect("suspend");
    assert!(
        ctx.service
            .find_authenticated_user(&user.id)
            .await
            .expect("suspended lookup")
            .is_none()
    );

    ctx.service.delete(&user.id).await.expect("cleanup");
}

#[tokio::test]
async fn find_with_sessions_and_activity_count_open_sessions() {
    let Some(ctx) = setup().await else {
        return;
    };
    let url = fixture_database_url().expect("url");
    let user = create_user(&ctx, "withsess").await;
    let pool = fixture_db_pool(&url).await.expect("pool");
    let s1 = SessionId::generate();
    let s2 = SessionId::generate();
    seed_user_session(&pool, &user.id, &s1).await.expect("s1");
    seed_user_session(&pool, &user.id, &s2).await.expect("s2");
    ctx.service.end_session(&s2).await.expect("end s2");

    let with_sessions = ctx
        .service
        .find_with_sessions(&user.id)
        .await
        .expect("query")
        .expect("row");
    assert_eq!(with_sessions.active_sessions, 1);
    assert!(with_sessions.last_session_at.is_some());

    let activity = ctx.service.get_activity(&user.id).await.expect("activity");
    assert_eq!(activity.session_count, 2);
    assert_eq!(activity.task_count, 0);
    assert_eq!(activity.message_count, 0);

    let listed = ctx
        .service
        .list_non_anonymous_with_sessions(100)
        .await
        .expect("non-anon list");
    assert!(listed.iter().any(|u| u.id == user.id));

    ctx.service.end_all_sessions(&user.id).await.expect("end");
    ctx.service.delete(&user.id).await.expect("cleanup");
}

#[tokio::test]
async fn list_search_and_count_reflect_created_users() {
    let Some(ctx) = setup().await else {
        return;
    };
    let user = create_user(&ctx, "listable").await;

    let listed = ctx.service.list(10_000, 0).await.expect("list");
    assert!(listed.iter().any(|u| u.id == user.id));

    let all = ctx.service.list_all().await.expect("list all");
    assert!(all.iter().any(|u| u.id == user.id));

    let found = ctx.service.search(&user.name, 50).await.expect("search");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, user.id);

    let by_email = ctx
        .service
        .search(&user.email, 50)
        .await
        .expect("search email");
    assert!(by_email.iter().any(|u| u.id == user.id));

    let total = ctx.service.count().await.expect("count");
    assert!(total >= 1);

    ctx.service.delete(&user.id).await.expect("cleanup");
}

#[tokio::test]
async fn list_by_filter_applies_status_role_and_age() {
    let Some(ctx) = setup().await else {
        return;
    };
    let user = create_user(&ctx, "filter").await;
    ctx.service
        .update_status(&user.id, UserStatus::Suspended)
        .await
        .expect("suspend");
    backdate_created_at(&ctx, &user.id, 10).await;

    let matched = ctx
        .service
        .list_by_filter(Some("suspended"), Some("user"), Some(5), 10_000)
        .await
        .expect("filter");
    assert!(matched.iter().any(|u| u.id == user.id));

    let too_old_cutoff = ctx
        .service
        .list_by_filter(Some("suspended"), None, Some(30), 10_000)
        .await
        .expect("age filter");
    assert!(!too_old_cutoff.iter().any(|u| u.id == user.id));

    let wrong_role = ctx
        .service
        .list_by_filter(None, Some("anonymous"), None, 10_000)
        .await
        .expect("role filter");
    assert!(!wrong_role.iter().any(|u| u.id == user.id));

    ctx.service.delete(&user.id).await.expect("cleanup");
}

#[tokio::test]
async fn bulk_update_status_and_bulk_delete() {
    let Some(ctx) = setup().await else {
        return;
    };
    let a = create_user(&ctx, "bulk-a").await;
    let b = create_user(&ctx, "bulk-b").await;
    let ids = vec![a.id.clone(), b.id.clone()];

    let updated = ctx
        .service
        .bulk_update_status(&ids, "suspended")
        .await
        .expect("bulk status");
    assert_eq!(updated, 2);
    let refreshed = ctx
        .service
        .find_by_id(&a.id)
        .await
        .expect("find")
        .expect("row");
    assert_eq!(refreshed.status.as_deref(), Some("suspended"));

    let deleted = ctx.service.bulk_delete(&ids).await.expect("bulk delete");
    assert_eq!(deleted, 2);
    assert!(
        ctx.service
            .find_by_id(&a.id)
            .await
            .expect("gone a")
            .is_none()
    );
    assert!(
        ctx.service
            .find_by_id(&b.id)
            .await
            .expect("gone b")
            .is_none()
    );
}

#[tokio::test]
async fn update_display_name_persists() {
    let Some(ctx) = setup().await else {
        return;
    };
    let user = create_user(&ctx, "disp").await;
    let updated = ctx
        .service
        .update_display_name(&user.id, "New Display")
        .await
        .expect("update");
    assert_eq!(updated.display_name.as_deref(), Some("New Display"));
    ctx.service.delete(&user.id).await.expect("cleanup");
}

#[tokio::test]
async fn missing_user_yields_not_found_across_mutations() {
    let Some(ctx) = setup().await else {
        return;
    };
    let ghost = UserId::new(Uuid::new_v4().to_string());

    assert!(matches!(
        ctx.service.update_email(&ghost, "x@y.invalid").await,
        Err(UserError::NotFound(_))
    ));
    assert!(matches!(
        ctx.service.update_full_name(&ghost, "Ghost").await,
        Err(UserError::NotFound(_))
    ));
    assert!(matches!(
        ctx.service.update_status(&ghost, UserStatus::Active).await,
        Err(UserError::NotFound(_))
    ));
    assert!(matches!(
        ctx.service.update_email_verified(&ghost, true).await,
        Err(UserError::NotFound(_))
    ));
    assert!(matches!(
        ctx.service.update_display_name(&ghost, "Ghost").await,
        Err(UserError::NotFound(_))
    ));
    assert!(matches!(
        ctx.service.assign_roles(&ghost, &["user".to_owned()]).await,
        Err(UserError::NotFound(_))
    ));
    assert!(matches!(
        ctx.service.delete(&ghost).await,
        Err(UserError::NotFound(_))
    ));
    assert!(matches!(
        ctx.service.is_temporary_anonymous(&ghost).await,
        Err(UserError::NotFound(_))
    ));
}

#[tokio::test]
async fn merge_users_transfers_sessions_and_removes_source() {
    let Some(ctx) = setup().await else {
        return;
    };
    let url = fixture_database_url().expect("url");
    let source = create_user(&ctx, "merge-src").await;
    let target = create_user(&ctx, "merge-dst").await;
    let pool = fixture_db_pool(&url).await.expect("pool");
    let sid = SessionId::generate();
    seed_user_session(&pool, &source.id, &sid)
        .await
        .expect("session");

    let result = ctx
        .service
        .merge_users(&source.id, &target.id)
        .await
        .expect("merge");
    assert_eq!(result.sessions_transferred, 1);
    assert_eq!(result.tasks_transferred, 0);

    assert!(
        ctx.service
            .find_by_id(&source.id)
            .await
            .expect("src")
            .is_none()
    );
    let sessions = ctx
        .service
        .list_sessions(&target.id)
        .await
        .expect("sessions");
    assert!(sessions.iter().any(|s| s.session_id == sid));

    ctx.service.end_all_sessions(&target.id).await.expect("end");
    ctx.service.delete(&target.id).await.expect("cleanup");
}

#[tokio::test]
async fn cleanup_old_anonymous_spares_users_with_open_sessions() {
    let Some(ctx) = setup().await else {
        return;
    };
    let url = fixture_database_url().expect("url");
    let stale = ctx
        .service
        .create_anonymous(&format!("stale-{}", Uuid::new_v4().simple()))
        .await
        .expect("stale anon");
    let kept = ctx
        .service
        .create_anonymous(&format!("kept-{}", Uuid::new_v4().simple()))
        .await
        .expect("kept anon");
    backdate_created_at(&ctx, &stale.id, 90).await;
    backdate_created_at(&ctx, &kept.id, 90).await;

    assert!(
        ctx.service
            .is_temporary_anonymous(&stale.id)
            .await
            .expect("anon")
    );

    let pool = fixture_db_pool(&url).await.expect("pool");
    let sid = SessionId::generate();
    seed_user_session(&pool, &kept.id, &sid)
        .await
        .expect("session");

    let removed = ctx
        .service
        .cleanup_old_anonymous(30)
        .await
        .expect("cleanup");
    assert!(removed >= 1);
    assert!(
        ctx.service
            .find_by_id(&stale.id)
            .await
            .expect("stale")
            .is_none()
    );
    assert!(
        ctx.service
            .find_by_id(&kept.id)
            .await
            .expect("kept")
            .is_some()
    );

    ctx.service.end_all_sessions(&kept.id).await.expect("end");
    ctx.service.delete(&kept.id).await.expect("cleanup kept");
}

#[tokio::test]
async fn create_anonymous_reuses_existing_fingerprint_row() {
    let Some(ctx) = setup().await else {
        return;
    };
    let fingerprint = format!("fp-{}", Uuid::new_v4().simple());
    let first = ctx
        .service
        .create_anonymous(&fingerprint)
        .await
        .expect("first");
    let second = ctx
        .service
        .create_anonymous(&fingerprint)
        .await
        .expect("second");
    assert_eq!(first.id, second.id);
    assert!(first.roles.iter().any(|r| r == "anonymous"));

    ctx.service.delete(&first.id).await.expect("cleanup");
}

#[tokio::test]
async fn stats_and_breakdowns_reflect_active_user_population() {
    let Some(ctx) = setup().await else {
        return;
    };
    let user = create_user(&ctx, "stats").await;

    let stats = ctx.service.get_stats().await.expect("stats");
    assert!(stats.total >= 1);
    assert!(stats.active >= 1);
    assert!(stats.created_24h >= 1);
    assert!(stats.created_7d >= stats.created_24h);
    assert!(stats.created_30d >= stats.created_7d);
    assert!(stats.newest_user.is_some());
    assert!(stats.oldest_user.is_some());

    let breakdown = ctx.service.count_with_breakdown().await.expect("breakdown");
    assert!(breakdown.total >= 1);
    assert!(*breakdown.by_status.get("active").unwrap_or(&0) >= 1);
    assert!(*breakdown.by_role.get("user").unwrap_or(&0) >= 1);

    ctx.service.delete(&user.id).await.expect("cleanup");
}
