use chrono::{TimeZone, Utc};
use systemprompt_analytics::resource_metrics::{ResourceFact, aggregate};
use systemprompt_identifiers::{AiRequestId, ResourceInvocationId, SessionId, UserId};

fn fact(
    user: &str,
    session: &str,
    invocation: &str,
    request: Option<&str>,
    at: i64,
) -> ResourceFact {
    ResourceFact {
        invocation_id: ResourceInvocationId::new(invocation),
        user_id: UserId::new(user),
        session_id: SessionId::new(session),
        invoked_at: Utc.timestamp_opt(at, 0).single().unwrap(),
        request_id: request.map(|id| AiRequestId::new(id)),
        input_tokens: None,
        output_tokens: None,
        cache_read_tokens: None,
        cache_creation_tokens: None,
        cost_microdollars: None,
        latency_ms: None,
        failed: false,
        quality_score: None,
        successful: None,
        revision_verified: false,
    }
}

#[test]
fn aggregate_counts_unique_scoped_facts_and_charges_a_duplicate_request_once() {
    let mut first = fact(
        "user-a",
        "conversation-a",
        "invocation-1",
        Some("request-1"),
        10,
    );
    first.input_tokens = Some(10);
    first.output_tokens = Some(20);
    first.cache_read_tokens = Some(2);
    first.cache_creation_tokens = Some(3);
    first.cost_microdollars = Some(100);
    first.latency_ms = Some(80);
    first.quality_score = Some(0.4);
    first.successful = Some(false);

    let mut duplicate_request = fact(
        "user-a",
        "conversation-a",
        "invocation-2",
        Some("request-1"),
        20,
    );
    duplicate_request.input_tokens = Some(1_000);
    duplicate_request.output_tokens = Some(2_000);
    duplicate_request.cost_microdollars = Some(9_999);
    duplicate_request.latency_ms = Some(1);
    duplicate_request.failed = true;
    duplicate_request.quality_score = Some(0.9);
    duplicate_request.successful = Some(true);
    duplicate_request.revision_verified = true;

    let mut failed = fact(
        "user-a",
        "conversation-b",
        "invocation-3",
        Some("request-2"),
        30,
    );
    failed.failed = true;
    failed.quality_score = None;
    failed.successful = Some(false);
    failed.revision_verified = true;

    let mut other_user = fact("user-b", "conversation-a", "invocation-1", None, 40);
    other_user.quality_score = Some(0.7);
    other_user.successful = Some(true);
    other_user.revision_verified = true;

    let duplicate_invocation = fact("user-a", "conversation-b", "invocation-1", None, 35);

    let metrics = aggregate([
        &first,
        &duplicate_request,
        &failed,
        &other_user,
        &duplicate_invocation,
    ]);

    assert_eq!(metrics.invocations, 4);
    assert_eq!(metrics.verified_invocations, 3);
    assert_eq!(metrics.users, 2);
    assert_eq!(metrics.conversations, 3);
    assert_eq!(metrics.requests, 2);
    assert_eq!(metrics.measured_requests, 1);
    assert_eq!(metrics.priced_requests, 1);
    assert_eq!(metrics.failed_requests, 1);
    assert_eq!(metrics.input_tokens, 10);
    assert_eq!(metrics.output_tokens, 20);
    assert_eq!(metrics.cache_read_tokens, 2);
    assert_eq!(metrics.cache_creation_tokens, 3);
    assert_eq!(metrics.related_cost_microdollars, Some(100));
    assert_eq!(metrics.average_tokens_per_measured_request, Some(35.0));
    assert_eq!(metrics.average_latency_ms, Some(80.0));
    assert_eq!(metrics.assessed_conversations, 3);
    assert_eq!(metrics.successful_conversations, 2);
    assert_eq!(metrics.average_quality_score, Some(0.8));
    assert_eq!(
        metrics.last_used_at,
        Some(Utc.timestamp_opt(40, 0).single().unwrap())
    );
}

#[test]
fn aggregate_keeps_zero_measurements_distinct_from_missing_or_negative_values() {
    let mut zero = fact("user", "conversation", "zero", Some("zero-request"), 10);
    zero.input_tokens = Some(0);
    zero.output_tokens = Some(0);
    zero.cache_read_tokens = Some(0);
    zero.cache_creation_tokens = Some(0);
    zero.cost_microdollars = Some(0);
    zero.latency_ms = Some(0);

    let mut missing = fact(
        "user",
        "conversation",
        "missing",
        Some("missing-request"),
        20,
    );
    missing.latency_ms = None;

    let mut negative = fact(
        "user",
        "conversation",
        "negative",
        Some("negative-request"),
        30,
    );
    negative.input_tokens = Some(-1);
    negative.output_tokens = Some(5);
    negative.cache_read_tokens = Some(-2);
    negative.cache_creation_tokens = Some(-3);
    negative.cost_microdollars = Some(-1);
    negative.latency_ms = Some(40);

    let metrics = aggregate([&zero, &missing, &negative]);

    assert_eq!(metrics.requests, 3);
    assert_eq!(metrics.measured_requests, 1);
    assert_eq!(metrics.priced_requests, 1);
    assert_eq!(metrics.input_tokens, 0);
    assert_eq!(metrics.output_tokens, 0);
    assert_eq!(metrics.cache_read_tokens, 0);
    assert_eq!(metrics.cache_creation_tokens, 0);
    assert_eq!(metrics.related_cost_microdollars, Some(0));
    assert_eq!(metrics.average_tokens_per_measured_request, Some(0.0));
    assert_eq!(metrics.average_latency_ms, Some(20.0));
    assert_eq!(metrics.average_quality_score, None);
}

#[test]
fn aggregate_keeps_an_empty_cohort_unmeasured_and_without_related_spend() {
    let metrics = aggregate(std::iter::empty::<&ResourceFact>());

    assert_eq!(metrics.invocations, 0);
    assert_eq!(metrics.users, 0);
    assert_eq!(metrics.conversations, 0);
    assert_eq!(metrics.requests, 0);
    assert_eq!(metrics.assessed_conversations, 0);
    assert_eq!(metrics.related_cost_microdollars, None);
    assert_eq!(metrics.average_tokens_per_measured_request, None);
    assert_eq!(metrics.average_latency_ms, None);
    assert_eq!(metrics.average_quality_score, None);
    assert_eq!(metrics.last_used_at, None);
}
