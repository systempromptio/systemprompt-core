//! DB-backed tests for `UserRateLimitBucketRepository`: concurrent hits on
//! one key count exactly, and pruning removes only elapsed windows.

use chrono::{Duration, Utc};
use systemprompt_database::DbPool;
use systemprompt_identifiers::UserId;
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_database_url, fixture_db_pool};
use systemprompt_users::UserRateLimitBucketRepository;
use uuid::Uuid;

async fn repo() -> Option<(UserRateLimitBucketRepository, DbPool)> {
    let url = fixture_database_url().ok()?;
    ensure_test_bootstrap();
    let db = fixture_db_pool(&url).await.expect("pool");
    let repo = UserRateLimitBucketRepository::new(&db).expect("repo");
    Some((repo, db))
}

// `prune` is global by window, so a cutoff at or after "now" would wipe the
// live rows of every test sharing the database. Tests remove their own rows
// by user id instead and only ever prune windows that are days old.
async fn cleanup(db: &DbPool, user: &UserId) {
    let pg = db.write_pool_arc().expect("write pool");
    sqlx::query("DELETE FROM user_rate_limit_buckets WHERE user_id = $1")
        .bind(user.as_str())
        .execute(&*pg)
        .await
        .expect("cleanup buckets");
}

fn user() -> UserId {
    UserId::new(format!("rl-bucket-{}", Uuid::new_v4().simple()))
}

#[tokio::test]
async fn thirty_two_concurrent_hits_on_one_key_sum_to_exactly_thirty_two() {
    let Some((repo, db)) = repo().await else {
        return;
    };
    let user = user();
    let window = Utc::now();

    let mut tasks = Vec::new();
    for _ in 0..32 {
        let repo = repo.clone();
        let user = user.clone();
        tasks.push(tokio::spawn(async move {
            repo.hit(&user, "test", window).await.expect("hit")
        }));
    }
    let mut seen = Vec::new();
    for task in tasks {
        seen.push(task.await.expect("join"));
    }
    seen.sort_unstable();

    assert_eq!(
        seen,
        (1..=32).collect::<Vec<i64>>(),
        "every hit must observe a distinct count; a lost update would repeat one"
    );
    let next = repo.hit(&user, "test", window).await.expect("hit");
    assert_eq!(next, 33);

    cleanup(&db, &user).await;
}

#[tokio::test]
async fn scopes_and_windows_count_independently() {
    let Some((repo, db)) = repo().await else {
        return;
    };
    let user = user();
    let window = Utc::now();
    let later = window + Duration::seconds(10);

    repo.hit(&user, "a", window).await.expect("hit");
    repo.hit(&user, "a", window).await.expect("hit");
    let other_scope = repo.hit(&user, "b", window).await.expect("hit");
    let other_window = repo.hit(&user, "a", later).await.expect("hit");

    assert_eq!(other_scope, 1, "a different scope starts its own counter");
    assert_eq!(other_window, 1, "a later window starts its own counter");

    cleanup(&db, &user).await;
}

#[tokio::test]
async fn prune_removes_only_windows_before_the_cutoff() {
    let Some((repo, db)) = repo().await else {
        return;
    };
    let user = user();
    let old = Utc::now() - Duration::days(400);
    let recent = Utc::now();

    repo.hit(&user, "prune", old).await.expect("hit");
    repo.hit(&user, "prune", recent).await.expect("hit");

    let removed = repo.prune(old + Duration::seconds(1)).await.expect("prune");
    assert!(removed >= 1, "the elapsed window must be removed");

    let survivor = repo.hit(&user, "prune", recent).await.expect("hit");
    assert_eq!(survivor, 2, "the current window keeps its count");
    let revived = repo.hit(&user, "prune", old).await.expect("hit");
    assert_eq!(revived, 1, "the pruned window restarts from zero");

    cleanup(&db, &user).await;
}
