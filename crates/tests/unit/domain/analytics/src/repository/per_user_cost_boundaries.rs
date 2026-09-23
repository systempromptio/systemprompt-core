use chrono::{Duration, Utc};
use systemprompt_analytics::CostAnalyticsRepository;
use systemprompt_identifiers::{ContextId, UserId};
use systemprompt_test_fixtures::{DisposableDb, seed_user_row};
use uuid::Uuid;

#[tokio::test]
async fn per_user_cost_context_queries_exclude_other_users_and_synthetic_requests() {
    let database = DisposableDb::installed("analytics_per_user_cost_boundaries")
        .await
        .expect("isolated analytics database");
    let pool = database.pool().await.expect("analytics database pool");
    let raw = pool.write_pool_arc().expect("analytics write pool");
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
    let base = Utc::now() - Duration::minutes(5);
    let window_end = base + Duration::minutes(10);

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

    for (index, user, context, model, cost, synthetic, created_at) in [
        (1, &owner, &first_context, "model-a", 110_i64, false, base),
        (
            2,
            &owner,
            &recent_context,
            "model-b",
            220,
            false,
            base + Duration::minutes(1),
        ),
        (
            3,
            &owner,
            &recent_context,
            "model-b",
            9_000,
            true,
            base + Duration::minutes(2),
        ),
        (
            4,
            &other,
            &other_context,
            "model-private",
            8_000,
            false,
            base + Duration::minutes(3),
        ),
        (
            5,
            &owner,
            &boundary_context,
            "model-at-end",
            7_000,
            false,
            window_end,
        ),
    ] {
        sqlx::query(
            "INSERT INTO ai_requests (id, request_id, user_id, context_id, provider, model, \
             tokens_used, cost_microdollars, status, actor_kind, actor_id, synthetic, created_at) \
             VALUES ($1, $2, $3, $4, 'fixture', $5, 10, $6, 'completed', 'user', $3, $7, $8)",
        )
        .bind(format!("cost-row-{suffix}-{index}"))
        .bind(format!("cost-request-{suffix}-{index}"))
        .bind(user.as_str())
        .bind(context.as_str())
        .bind(model)
        .bind(cost)
        .bind(synthetic)
        .bind(created_at)
        .execute(raw.as_ref())
        .await
        .expect("insert reporting request");
    }

    let repository = CostAnalyticsRepository::new(&pool).expect("cost repository");
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

    drop(repository);
    drop(raw);
    pool.write_pool_arc().expect("write pool").close().await;
    drop(pool);
    database.drop_now().await;
}
