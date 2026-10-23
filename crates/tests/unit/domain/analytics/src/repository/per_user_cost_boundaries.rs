use chrono::{Duration, DurationRound, Utc};
use systemprompt_analytics::CostAnalyticsRepository;
use systemprompt_identifiers::{ContextId, UserId};
use systemprompt_test_fixtures::{DisposableDb, seed_user_row};
use uuid::Uuid;

#[tokio::test]
async fn per_user_cost_context_queries_exclude_other_users_and_evaluation_traffic() {
    let database = DisposableDb::with_schema("analytics_per_user_cost_boundaries").await;
    let pool = database.test_pool().await;
    let raw = pool.write_pool();
    let suffix = Uuid::new_v4().simple().to_string();
    let owner = UserId::new(format!("cost-owner-{suffix}"));
    let other = UserId::new(format!("cost-other-{suffix}"));
    seed_user_row(&pool, &owner, &format!("{}@cost.invalid", owner.as_str()))
        .await
        .expect("seed report owner");
    seed_user_row(&pool, &other, &format!("{}@cost.invalid", other.as_str()))
        .await
        .expect("seed other report user");
    let first_context = ContextId::generate();
    let recent_context = ContextId::generate();
    let other_context = ContextId::generate();
    let boundary_context = ContextId::generate();
    let window_end = Utc::now()
        .duration_trunc(Duration::hours(1))
        .expect("truncate to the hour");
    let base = window_end - Duration::minutes(10);

    for (context, user, name) in [
        (&first_context, &owner, "owner first"),
        (&recent_context, &owner, "owner recent"),
        (&other_context, &other, "other private"),
        (&boundary_context, &owner, "owner boundary"),
    ] {
        sqlx::query(
            "INSERT INTO user_contexts (context_id, user_id, name, kind, created_at, updated_at) \
             VALUES ($1, $2, $3, 'user', $4, $4)",
        )
        .bind(context.as_str())
        .bind(user.as_str())
        .bind(name)
        .bind(base)
        .execute(raw.as_ref())
        .await
        .expect("insert reporting context");
    }

    for (index, user, context, model, cost, traffic, created_at) in [
        (
            1,
            &owner,
            &first_context,
            "model-a",
            110_i64,
            "production",
            base,
        ),
        (
            2,
            &owner,
            &recent_context,
            "model-b",
            220,
            "production",
            base + Duration::minutes(1),
        ),
        (
            3,
            &owner,
            &recent_context,
            "model-b",
            9_000,
            "evaluation",
            base + Duration::minutes(2),
        ),
        (
            4,
            &other,
            &other_context,
            "model-private",
            8_000,
            "production",
            base + Duration::minutes(3),
        ),
        (
            5,
            &owner,
            &boundary_context,
            "model-at-end",
            7_000,
            "production",
            window_end,
        ),
    ] {
        sqlx::query(
            "INSERT INTO ai_requests (id, request_id, user_id, context_id, provider, model, \
             tokens_used, cost_microdollars, status, actor_kind, actor_id, traffic, created_at) \
             VALUES ($1, $2, $3, $4, 'fixture', $5, 10, $6, 'completed', 'user', $3, $7, $8)",
        )
        .bind(format!("cost-row-{suffix}-{index}"))
        .bind(format!("cost-request-{suffix}-{index}"))
        .bind(user.as_str())
        .bind(context.as_str())
        .bind(model)
        .bind(cost)
        .bind(traffic)
        .bind(created_at)
        .execute(raw.as_ref())
        .await
        .expect("insert reporting request");
    }

    let repository = CostAnalyticsRepository::new(&pool);
    let start = base - Duration::seconds(1);

    let previous = repository
        .get_previous_cost_for_user(&owner, start, window_end)
        .await
        .expect("previous owner cost");
    assert_eq!(previous.cost, Some(330));

    let contexts = repository
        .get_contexts_by_model_for_user(&owner, start, window_end, 10)
        .await
        .expect("owner contexts by model");
    assert_eq!(contexts.len(), 2);
    assert!(
        contexts
            .iter()
            .any(|row| { row.name == "model-a" && row.conversations == 1 && row.ai_requests == 1 })
    );
    assert!(
        contexts
            .iter()
            .any(|row| { row.name == "model-b" && row.conversations == 1 && row.ai_requests == 1 })
    );
    assert!(contexts.iter().all(|row| row.name != "model-private"));

    let recent = repository
        .get_recent_contexts_for_user(&owner, base + Duration::minutes(5), 1)
        .await
        .expect("recent owner context");
    assert_eq!(recent.len(), 1);
    assert_eq!(recent[0].context_id, recent_context);
    assert_eq!(recent[0].ai_requests, 1);
    assert_eq!(recent[0].model.as_deref(), Some("model-b"));
    assert_eq!(recent[0].context_name.as_deref(), Some("owner recent"));

    let rollup: Vec<(String, i64, i64)> = sqlx::query_as(
        "SELECT traffic, request_count, cost_microdollars FROM ai_usage_rollup_hourly \
         WHERE user_id = $1 AND bucket = $2 ORDER BY traffic, model",
    )
    .bind(owner.as_str())
    .bind(window_end - Duration::hours(1))
    .fetch_all(raw.as_ref())
    .await
    .expect("owner rollup rows");
    assert_eq!(
        rollup,
        vec![
            ("evaluation".to_owned(), 1, 9_000),
            ("production".to_owned(), 1, 110),
            ("production".to_owned(), 1, 220),
        ]
    );

    drop(repository);
    drop(raw);
    pool.write_pool().close().await;
    drop(pool);
    database.drop_now().await;
}
