// DB-backed tests for the `AiRequestTrace` seam over `AiRequestRepository`:
// usage reads are owner-scoped and sampling hydrates the stored turns.

use serde_json::json;
use systemprompt_ai::repository::{AiRequestPayloadRepository, AiRequestRepository};
use systemprompt_identifiers::ContextId;
use systemprompt_traits::{AiRequestTrace, TraceRequestStatus, TraceSampleFilter, TraceSampleMode};

use super::{completed_record, pool_or_skip, seed_request, user};

#[tokio::test]
async fn usage_reads_are_scoped_to_the_owner() {
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    let repo = AiRequestRepository::new(&pool).expect("repo");
    let owner = user();
    let stranger = user();
    let email = format!("{}@ai.invalid", owner.as_str());
    systemprompt_test_fixtures::seed_user_row(&pool, &owner, &email)
        .await
        .expect("seed");
    let id = repo
        .insert(&completed_record(&owner))
        .await
        .expect("insert");

    let usage = repo
        .find_usage(&owner, &id)
        .await
        .expect("find")
        .expect("owner sees the request");
    assert_eq!(usage.status, TraceRequestStatus::Completed);
    assert!(usage.is_settled());
    assert_eq!(usage.cost_microdollars, 1_500);
    assert_eq!(usage.tool_calls, 0);

    assert!(
        repo.find_usage(&stranger, &id)
            .await
            .expect("find")
            .is_none(),
        "another user's request is reported as absent"
    );
    assert!(
        repo.list_usage(&owner, &[])
            .await
            .expect("empty")
            .is_empty()
    );
}

#[tokio::test]
async fn sample_by_id_hydrates_turns_and_splits_the_response() {
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    let repo = AiRequestRepository::new(&pool).expect("repo");
    let owner = user();
    let email = format!("{}@ai.invalid", owner.as_str());
    systemprompt_test_fixtures::seed_user_row(&pool, &owner, &email)
        .await
        .expect("seed");
    let id = repo
        .insert(&completed_record(&owner))
        .await
        .expect("insert");
    repo.insert_message(&id, "user", "hello", 0)
        .await
        .expect("user turn");
    repo.insert_message(&id, "assistant", "hi there", 1)
        .await
        .expect("assistant turn");
    let pending = seed_request(&pool, &owner).await;

    let filter = TraceSampleFilter::with_limit(10).ids(vec![id.clone(), pending.clone()]);
    let samples = repo.sample(&filter).await.expect("sample");

    assert_eq!(samples.len(), 1, "only completed rows are sampled");
    let sample = &samples[0];
    assert_eq!(sample.ai_request_id, id);
    assert_eq!(sample.provider.as_str(), "anthropic");
    assert_eq!(sample.messages.len(), 1);
    assert_eq!(sample.messages[0].content, "hello");
    assert_eq!(sample.response_text.as_deref(), Some("hi there"));
}

#[tokio::test]
async fn conversation_sampling_selects_the_latest_turn_and_keeps_its_wire_evidence() {
    let pool = pool_or_skip().await.expect("AI trace fixture database");
    let repo = AiRequestRepository::new(&pool).unwrap();
    let owner = user();
    systemprompt_test_fixtures::seed_user_row(&pool, &owner, &format!("{owner}@ai.invalid"))
        .await
        .unwrap();
    let context = ContextId::generate();
    let mut earlier = completed_record(&owner);
    earlier.context_id = context.clone();
    let earlier_id = repo.insert(&earlier).await.unwrap();
    let mut latest = completed_record(&owner);
    latest.context_id = context.clone();
    let latest_id = repo.insert(&latest).await.unwrap();
    sqlx::query("UPDATE ai_requests SET created_at = NOW() - INTERVAL '1 minute' WHERE id = $1")
        .bind(earlier_id.as_str())
        .execute(pool.write_pool_arc().unwrap().as_ref())
        .await
        .unwrap();
    repo.insert_message(&latest_id, "user", "latest question", 0)
        .await
        .unwrap();
    repo.insert_message(&latest_id, "assistant", "latest answer", 1)
        .await
        .unwrap();
    repo.update_system_prompt_override(&latest_id, "policy-v2")
        .await
        .unwrap();
    let tools = json!([{"name":"lookup"}]);
    AiRequestPayloadRepository::new(&pool)
        .unwrap()
        .upsert_offered_tools(&latest_id, &tools)
        .await
        .unwrap();
    AiRequestPayloadRepository::new(&pool)
        .unwrap()
        .upsert_prepared(&latest_id, "prepared-latest", None)
        .await
        .unwrap();

    let samples = repo
        .sample(
            &TraceSampleFilter::with_limit(5)
                .mode(TraceSampleMode::Conversation)
                .context_id(context),
        )
        .await
        .unwrap();

    assert_eq!(samples.len(), 1);
    assert_eq!(samples[0].ai_request_id, latest_id);
    assert_eq!(samples[0].messages[0].content, "latest question");
    assert_eq!(samples[0].response_text.as_deref(), Some("latest answer"));
    assert_eq!(samples[0].offered_tools, Some(tools));
    assert_eq!(
        samples[0].prepared_body_sha256.as_deref(),
        Some("prepared-latest")
    );
    assert_eq!(
        samples[0].system_prompt_override.as_deref(),
        Some("policy-v2")
    );
}
