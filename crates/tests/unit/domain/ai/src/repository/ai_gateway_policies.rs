// DB-backed tests for AiGatewayPolicyRepository upsert / list / delete.

use serde_json::json;
use systemprompt_ai::repository::AiGatewayPolicyRepository;
use uuid::Uuid;

use super::pool;

async fn repo() -> Option<AiGatewayPolicyRepository> {
    let pool = pool().await?;
    Some(AiGatewayPolicyRepository::new(&pool).expect("repo"))
}

fn unique_name() -> String {
    format!("policy-{}", Uuid::new_v4())
}

#[tokio::test]
async fn upsert_inserts_then_updates_same_name() {
    let Some(repo) = repo().await else {
        return;
    };
    let name = unique_name();
    let spec = json!({"block_categories": ["pii"]});
    let id1 = repo.upsert(&name, &spec, true).await.expect("insert");

    let spec2 = json!({"block_categories": ["pii", "malware"]});
    let id2 = repo.upsert(&name, &spec2, false).await.expect("update");
    // ON CONFLICT (name) keeps the original row id.
    assert_eq!(id1, id2);
}

#[tokio::test]
async fn find_for_global_returns_only_enabled() {
    let Some(repo) = repo().await else {
        return;
    };
    let enabled = unique_name();
    let disabled = unique_name();
    repo.upsert(&enabled, &json!({}), true).await.expect("enabled");
    repo.upsert(&disabled, &json!({}), false)
        .await
        .expect("disabled");

    let rows = repo.find_for_global().await.expect("find");
    assert!(rows.iter().any(|r| r.name == enabled && r.enabled));
    assert!(!rows.iter().any(|r| r.name == disabled));
}

#[tokio::test]
async fn list_all_names_includes_disabled() {
    let Some(repo) = repo().await else {
        return;
    };
    let name = unique_name();
    repo.upsert(&name, &json!({}), false).await.expect("upsert");
    let names = repo.list_all_names().await.expect("list");
    assert!(names.contains(&name));
}

#[tokio::test]
async fn delete_by_name_removes_policy() {
    let Some(repo) = repo().await else {
        return;
    };
    let name = unique_name();
    repo.upsert(&name, &json!({}), true).await.expect("upsert");
    repo.delete_by_name(&name).await.expect("delete");
    let names = repo.list_all_names().await.expect("list");
    assert!(!names.contains(&name));
}
