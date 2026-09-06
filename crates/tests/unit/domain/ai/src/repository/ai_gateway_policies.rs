// DB-backed tests for AiGatewayPolicyRepository upsert / list / delete.

use serde_json::json;
use systemprompt_ai::repository::AiGatewayPolicyRepository;
use uuid::Uuid;

use super::pool_or_skip;

async fn repo_or_skip() -> Option<AiGatewayPolicyRepository> {
    let pool = pool_or_skip().await?;
    Some(AiGatewayPolicyRepository::new(&pool).expect("repo"))
}

fn unique_name() -> String {
    format!("policy-{}", Uuid::new_v4())
}

#[tokio::test]
async fn upsert_inserts_then_updates_same_name() {
    let Some(repo) = repo_or_skip().await else {
        return;
    };
    let name = unique_name();
    let spec = json!({"block_categories": ["pii"]});
    let id1 = repo.upsert(&name, &spec, true, 0).await.expect("insert");

    let spec2 = json!({"block_categories": ["pii", "malware"]});
    let id2 = repo.upsert(&name, &spec2, false, 0).await.expect("update");
    // ON CONFLICT (name) keeps the original row id.
    assert_eq!(id1, id2);
}

#[tokio::test]
async fn find_for_global_returns_only_enabled() {
    let Some(repo) = repo_or_skip().await else {
        return;
    };
    let enabled = unique_name();
    let disabled = unique_name();
    repo.upsert(&enabled, &json!({}), true, 0)
        .await
        .expect("enabled");
    repo.upsert(&disabled, &json!({}), false, 0)
        .await
        .expect("disabled");

    let rows = repo.list_for_global().await.expect("find");
    assert!(rows.iter().any(|r| r.name == enabled && r.enabled));
    assert!(!rows.iter().any(|r| r.name == disabled));
}

#[tokio::test]
async fn list_all_names_includes_disabled() {
    let Some(repo) = repo_or_skip().await else {
        return;
    };
    let name = unique_name();
    repo.upsert(&name, &json!({}), false, 0)
        .await
        .expect("upsert");
    let names = repo.list_all_names().await.expect("list");
    assert!(names.contains(&name));
}

#[tokio::test]
async fn delete_by_name_removes_policy() {
    let Some(repo) = repo_or_skip().await else {
        return;
    };
    let name = unique_name();
    repo.upsert(&name, &json!({}), true, 0)
        .await
        .expect("upsert");
    repo.delete_by_name(&name).await.expect("delete");
    let names = repo.list_all_names().await.expect("list");
    assert!(!names.contains(&name));
}

#[tokio::test]
async fn upsert_persists_priority_and_orders_ascending() {
    let Some(repo) = repo_or_skip().await else {
        return;
    };
    let low = unique_name();
    let high = unique_name();
    repo.upsert(&low, &json!({}), true, 10).await.expect("low");
    repo.upsert(&high, &json!({}), true, 20)
        .await
        .expect("high");

    let rows = repo.list_for_global().await.expect("list");
    let low_at = rows.iter().position(|r| r.name == low).expect("low row");
    let high_at = rows.iter().position(|r| r.name == high).expect("high row");
    assert!(low_at < high_at);
    assert_eq!(rows[low_at].priority, 10);
    assert_eq!(rows[high_at].priority, 20);
}

#[tokio::test]
async fn upsert_updates_priority_on_conflict() {
    let Some(repo) = repo_or_skip().await else {
        return;
    };
    let name = unique_name();
    repo.upsert(&name, &json!({}), true, 5)
        .await
        .expect("insert");
    repo.upsert(&name, &json!({}), true, 42)
        .await
        .expect("update");

    let rows = repo.list_for_global().await.expect("list");
    let row = rows.iter().find(|r| r.name == name).expect("row");
    assert_eq!(row.priority, 42);
}
