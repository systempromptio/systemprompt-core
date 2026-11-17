//! `list_ai_requests` pages by the `(created_at, id)` cursor, so requests
//! that share one `created_at` page without a gap or a repeat, and a request
//! written between two page reads lands on the side of the cursor its
//! `(created_at, id)` puts it.

use chrono::{DateTime, Utc};
use systemprompt_runtime::{AiRequestFilter, RequestCursor, TraceQueryService, TraceRepository};
use systemprompt_test_fixtures::test_pg_pool;

async fn insert_user(pool: &sqlx::PgPool, user: &str) {
    sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $1, $2)")
        .bind(user)
        .bind(format!("{user}@test.invalid"))
        .execute(pool)
        .await
        .expect("user");
}

async fn insert_request(pool: &sqlx::PgPool, id: &str, user: &str, at: DateTime<Utc>) {
    sqlx::query(
        "INSERT INTO ai_requests \
         (id, request_id, user_id, context_id, provider, model, actor_kind, actor_id, status, \
          cost_microdollars, created_at) \
         VALUES ($1, $1, $2, '00000000-0000-0000-0000-00000000c0de', 'anthropic', \
                 'paging-model', 'user', $2, 'completed', 0, $3)",
    )
    .bind(id)
    .bind(user)
    .bind(at)
    .execute(pool)
    .await
    .expect("request");
}

async fn cleanup(pool: &sqlx::PgPool, users: &[&str]) {
    for user in users {
        let _ = sqlx::query("DELETE FROM ai_requests WHERE user_id = $1")
            .bind(user)
            .execute(pool)
            .await;
        let _ = sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(user)
            .execute(pool)
            .await;
    }
}

#[tokio::test]
async fn tied_timestamps_page_by_id_and_a_request_written_between_reads_keeps_its_side() {
    let pool = test_pg_pool().await;
    let tag = uuid::Uuid::new_v4().simple().to_string();
    let user = format!("paging_user_{tag}");
    let sibling = format!("paging_sibling_{tag}");
    insert_user(&pool, &user).await;
    insert_user(&pool, &sibling).await;
    let at: DateTime<Utc> = sqlx::query_scalar("SELECT now() - interval '1 hour'")
        .fetch_one(&pool)
        .await
        .expect("timestamp");
    let id = |suffix: &str| format!("paging_{tag}_{suffix}");
    insert_request(&pool, &id("b"), &user, at).await;
    insert_request(&pool, &id("d"), &user, at).await;

    let svc = TraceQueryService::new(TraceRepository::new(std::sync::Arc::new(pool.clone())));
    let first = svc
        .list_ai_requests(&AiRequestFilter::new(1).with_user(user.clone()))
        .await
        .expect("first page");
    let first_ids: Vec<&str> = first.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(first_ids, [id("d").as_str()]);

    insert_request(&pool, &id("c"), &user, at).await;
    insert_request(&pool, &id("e"), &user, at).await;
    insert_request(&pool, &id("a"), &sibling, at).await;

    let cursor = RequestCursor {
        created_at: first[0].created_at,
        id: first[0].id.clone(),
    };
    let second = svc
        .list_ai_requests(
            &AiRequestFilter::new(50)
                .with_user(user.clone())
                .with_before(cursor),
        )
        .await
        .expect("second page");
    let second_ids: Vec<&str> = second.iter().map(|r| r.id.as_str()).collect();
    cleanup(&pool, &[&user, &sibling]).await;

    assert_eq!(
        second_ids,
        [id("c").as_str(), id("b").as_str()],
        "rows tied on created_at continue past the cursor by id; e sorts before the cursor \
         and the sibling's row is another user's"
    );
}
