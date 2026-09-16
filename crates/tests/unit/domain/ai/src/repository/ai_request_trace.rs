// DB-backed tests for the `AiRequestTrace` seam over `AiRequestRepository`:
// usage reads are owner-scoped and sampling hydrates the stored turns.

use systemprompt_ai::repository::AiRequestRepository;
use systemprompt_traits::{AiRequestTrace, TraceRequestStatus, TraceSampleFilter};

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
