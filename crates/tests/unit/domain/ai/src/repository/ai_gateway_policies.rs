// DB-backed tests for AiGatewayPolicyRepository: the plane's connection-scoped
// list / upsert / delete and the request path's enabled-only read.

use super::bootstrapped_pool;
use serde_json::{Value, json};
use systemprompt_ai::repository::{AiGatewayPolicyRepository, GatewayPolicyWrite};
use systemprompt_test_fixtures::unique_name;

async fn upsert(
    pool: &systemprompt_database::DbPool,
    name: &str,
    spec: &Value,
    enabled: bool,
    priority: i32,
) {
    let mut conn = pool.write_pool().acquire().await.expect("conn");
    AiGatewayPolicyRepository::upsert_on(
        &mut conn,
        &GatewayPolicyWrite {
            name,
            spec,
            enabled,
            priority,
            source: "code:test",
        },
    )
    .await
    .expect("upsert");
}

#[tokio::test]
async fn upsert_inserts_then_updates_and_list_includes_disabled() {
    let pool = bootstrapped_pool().await;
    let name = unique_name("policy");
    upsert(&pool, &name, &json!({"a": 1}), true, 0).await;
    upsert(&pool, &name, &json!({"a": 2}), false, 42).await;

    let mut conn = pool.write_pool().acquire().await.expect("conn");
    let rows = AiGatewayPolicyRepository::list_all_on(&mut conn)
        .await
        .expect("list");
    let row = rows.iter().find(|r| r.name == name).expect("row");
    assert_eq!(
        (row.enabled, row.priority, row.spec.clone()),
        (false, 42, json!({"a": 2}))
    );
}

#[tokio::test]
async fn find_for_global_returns_only_enabled_in_priority_order() {
    let pool = bootstrapped_pool().await;
    let (low, high, off) = (
        unique_name("policy"),
        unique_name("policy"),
        unique_name("policy"),
    );
    upsert(&pool, &high, &json!({}), true, 20).await;
    upsert(&pool, &low, &json!({}), true, 10).await;
    upsert(&pool, &off, &json!({}), false, 0).await;

    let names: Vec<String> = AiGatewayPolicyRepository::new(&pool)
        .list_for_global()
        .await
        .expect("list")
        .into_iter()
        .map(|r| r.name)
        .collect();
    assert!(!names.contains(&off));
    let lo = names.iter().position(|n| *n == low).expect("low");
    let hi = names.iter().position(|n| *n == high).expect("high");
    assert!(lo < hi);
}

#[tokio::test]
async fn delete_on_removes_the_policy() {
    let pool = bootstrapped_pool().await;
    let name = unique_name("policy");
    upsert(&pool, &name, &json!({}), true, 0).await;
    let mut conn = pool.write_pool().acquire().await.expect("conn");
    AiGatewayPolicyRepository::delete_on(&mut conn, &name)
        .await
        .expect("delete");
    let rows = AiGatewayPolicyRepository::list_all_on(&mut conn)
        .await
        .expect("list");
    assert!(rows.iter().all(|r| r.name != name));
}
