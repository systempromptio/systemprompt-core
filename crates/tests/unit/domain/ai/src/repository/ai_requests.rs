// DB-backed tests for AiRequestRepository: insert, status updates, usage
// aggregates, and per-turn message / tool-call writes.

use systemprompt_ai::repository::ai_requests::UpdateCompletionParams;
use systemprompt_ai::repository::{AiRequestRepository, InsertToolCallParams};
use systemprompt_identifiers::AiRequestId;
use uuid::Uuid;

use super::{completed_record, pool, seed_request, user};

async fn repo() -> Option<(AiRequestRepository, systemprompt_database::DbPool)> {
    let pool = pool().await?;
    let repo = AiRequestRepository::new(&pool).expect("repo");
    Some((repo, pool))
}

#[tokio::test]
async fn insert_then_get_by_id_round_trips() {
    let Some((repo, pool)) = repo().await else {
        return;
    };
    let uid = user();
    let email = format!("{}@ai.invalid", uid.as_str());
    systemprompt_test_fixtures::seed_user_row(&pool, &uid, &email)
        .await
        .expect("seed");
    let record = completed_record(&uid);
    let id = repo.insert(&record).await.expect("insert");

    let fetched = repo.get_by_id(&id).await.expect("get").expect("present");
    assert_eq!(fetched.id, id);
    assert_eq!(fetched.user_id, uid);
    assert_eq!(fetched.provider, "anthropic");
    assert_eq!(fetched.model, "claude-3-opus");
    assert_eq!(fetched.status, "completed");
    assert_eq!(fetched.input_tokens, Some(100));
    assert_eq!(fetched.output_tokens, Some(50));
    assert!(fetched.cache_hit);
    assert_eq!(fetched.cost_microdollars, 1_500);
    assert!(fetched.completed_at.is_some());
}

#[tokio::test]
async fn get_by_id_missing_returns_none() {
    let Some((repo, _pool)) = repo().await else {
        return;
    };
    let missing = AiRequestId::generate();
    assert!(repo.get_by_id(&missing).await.expect("get").is_none());
}

#[tokio::test]
async fn insert_with_id_uses_supplied_id() {
    let Some((repo, pool)) = repo().await else {
        return;
    };
    let uid = user();
    let email = format!("{}@ai.invalid", uid.as_str());
    systemprompt_test_fixtures::seed_user_row(&pool, &uid, &email)
        .await
        .expect("seed");
    let id = AiRequestId::generate();
    let record = completed_record(&uid);
    let returned = repo.insert_with_id(&id, &record).await.expect("insert");
    assert_eq!(returned, id);
    assert!(repo.get_by_id(&id).await.expect("get").is_some());
}

#[tokio::test]
async fn update_completion_sets_tokens_and_status() {
    let Some((repo, pool)) = repo().await else {
        return;
    };
    let uid = user();
    let id = seed_request(&pool, &uid).await;

    let updated = repo
        .update_completion(UpdateCompletionParams {
            id: id.clone(),
            tokens_used: 300,
            input_tokens: 200,
            output_tokens: 100,
            cost_microdollars: 9_000,
            latency_ms: 750,
            cache_hit: true,
            cache_read_tokens: 128,
            cache_creation_tokens: 0,
        })
        .await
        .expect("update");
    assert_eq!(updated.id, id);
    assert_eq!(updated.status, "completed");
    assert_eq!(updated.tokens_used, Some(300));
    assert_eq!(updated.input_tokens, Some(200));
    assert_eq!(updated.output_tokens, Some(100));
    assert_eq!(updated.cost_microdollars, 9_000);
    assert_eq!(updated.latency_ms, Some(750));
    assert!(updated.cache_hit);
    assert_eq!(updated.cache_read_tokens, Some(128));
    assert!(updated.completed_at.is_some());
}

#[tokio::test]
async fn update_error_sets_failed_status_and_message() {
    let Some((repo, pool)) = repo().await else {
        return;
    };
    let uid = user();
    let id = seed_request(&pool, &uid).await;

    let updated = repo
        .update_error(&id, "provider exploded")
        .await
        .expect("update error");
    assert_eq!(updated.status, "failed");
    assert_eq!(updated.error_message.as_deref(), Some("provider exploded"));
    assert!(updated.completed_at.is_some());
}

#[tokio::test]
async fn update_model_changes_model() {
    let Some((repo, pool)) = repo().await else {
        return;
    };
    let uid = user();
    let id = seed_request(&pool, &uid).await;
    repo.update_model(&id, "claude-3-5-sonnet")
        .await
        .expect("update model");
    let fetched = repo.get_by_id(&id).await.expect("get").expect("present");
    assert_eq!(fetched.model, "claude-3-5-sonnet");
}

#[tokio::test]
async fn get_user_usage_aggregates_requests() {
    let Some((repo, pool)) = repo().await else {
        return;
    };
    let uid = user();
    let email = format!("{}@ai.invalid", uid.as_str());
    systemprompt_test_fixtures::seed_user_row(&pool, &uid, &email)
        .await
        .expect("seed");
    repo.insert(&completed_record(&uid))
        .await
        .expect("insert 1");
    repo.insert(&completed_record(&uid))
        .await
        .expect("insert 2");

    let usage = repo.get_user_usage(&uid).await.expect("usage");
    assert_eq!(usage.user_id, uid);
    assert_eq!(usage.request_count, 2);
    assert_eq!(usage.total_tokens, 300);
}

#[tokio::test]
async fn get_provider_usage_groups_by_provider_model() {
    let Some((repo, pool)) = repo().await else {
        return;
    };
    let uid = user();
    let email = format!("{}@ai.invalid", uid.as_str());
    systemprompt_test_fixtures::seed_user_row(&pool, &uid, &email)
        .await
        .expect("seed");
    repo.insert(&completed_record(&uid)).await.expect("insert");

    let usage = repo.get_provider_usage(30).await.expect("provider usage");
    // The aggregate spans the whole shard DB; our row must appear among them.
    assert!(
        usage
            .iter()
            .any(|u| u.provider == "anthropic" && u.model == "claude-3-opus")
    );
}

#[tokio::test]
async fn insert_and_get_messages_in_sequence_order() {
    let Some((repo, pool)) = repo().await else {
        return;
    };
    let uid = user();
    let id = seed_request(&pool, &uid).await;

    repo.insert_message(&id, "user", "hello", 0)
        .await
        .expect("msg 0");
    let m1 = repo
        .insert_message(&id, "assistant", "hi there", 1)
        .await
        .expect("msg 1");
    assert_eq!(m1.role, "assistant");
    assert_eq!(m1.content, "hi there");
    assert_eq!(m1.sequence_number, 1);

    let messages = repo.get_messages(&id).await.expect("messages");
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].sequence_number, 0);
    assert_eq!(messages[1].sequence_number, 1);
    assert_eq!(messages[0].content, "hello");
}

#[tokio::test]
async fn get_max_sequence_reflects_inserted_messages() {
    let Some((repo, pool)) = repo().await else {
        return;
    };
    let uid = user();
    let id = seed_request(&pool, &uid).await;
    assert_eq!(repo.get_max_sequence(&id).await.expect("empty"), 0);

    repo.insert_message(&id, "user", "a", 0).await.expect("0");
    repo.insert_message(&id, "user", "b", 5).await.expect("5");
    assert_eq!(repo.get_max_sequence(&id).await.expect("max"), 5);
}

#[tokio::test]
async fn add_response_message_appends_after_max() {
    let Some((repo, pool)) = repo().await else {
        return;
    };
    let uid = user();
    let id = seed_request(&pool, &uid).await;
    repo.insert_message(&id, "user", "q", 0).await.expect("0");
    repo.add_response_message(&id, "the answer")
        .await
        .expect("append");

    let messages = repo.get_messages(&id).await.expect("messages");
    assert_eq!(messages.len(), 2);
    let last = messages.last().expect("last");
    assert_eq!(last.role, "assistant");
    assert_eq!(last.content, "the answer");
    assert_eq!(last.sequence_number, 1);
}

#[tokio::test]
async fn insert_and_get_tool_calls() {
    let Some((repo, pool)) = repo().await else {
        return;
    };
    let uid = user();
    let id = seed_request(&pool, &uid).await;
    let call_id = Uuid::new_v4().to_string();

    let inserted = repo
        .insert_tool_call(InsertToolCallParams {
            request_id: &id,
            ai_tool_call_id: &call_id,
            tool_name: "search",
            tool_input: r#"{"q":"rust"}"#,
            sequence_number: 0,
        })
        .await
        .expect("insert tool call");
    assert_eq!(inserted.tool_name, "search");
    assert_eq!(inserted.sequence_number, 0);

    let calls = repo.get_tool_calls(&id).await.expect("tool calls");
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].tool_name, "search");
    assert_eq!(calls[0].tool_input, r#"{"q":"rust"}"#);
    assert!(calls[0].mcp_execution_id.is_none());
}

#[tokio::test]
async fn link_tool_calls_empty_input_returns_zero() {
    let Some((repo, _pool)) = repo().await else {
        return;
    };
    let affected = repo
        .link_tool_calls_to_recent_executions(&[])
        .await
        .expect("link");
    assert_eq!(affected, 0);
}

#[tokio::test]
async fn link_tool_calls_no_matching_executions_affects_zero() {
    let Some((repo, pool)) = repo().await else {
        return;
    };
    let uid = user();
    let id = seed_request(&pool, &uid).await;
    let call_id = Uuid::new_v4().to_string();
    repo.insert_tool_call(InsertToolCallParams {
        request_id: &id,
        ai_tool_call_id: &call_id,
        tool_name: "noop",
        tool_input: "{}",
        sequence_number: 0,
    })
    .await
    .expect("insert");

    // No mcp_tool_executions row references this call id, so nothing links.
    let affected = repo
        .link_tool_calls_to_recent_executions(&[call_id])
        .await
        .expect("link");
    assert_eq!(affected, 0);
}
