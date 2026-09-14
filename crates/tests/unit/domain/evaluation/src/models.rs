use chrono::Utc;
use systemprompt_evaluation::{CanonicalMessage, SampleFilter, SampledRequest};
use systemprompt_identifiers::{AiRequestId, ContextId};

#[test]
fn sample_filter_builder_sets_fields() {
    let since = Utc::now();
    let filter = SampleFilter::with_limit(7)
        .since(since)
        .provider("anthropic")
        .model("claude-sonnet-5")
        .ids(vec!["a".to_owned()]);
    assert_eq!(filter.limit, 7);
    assert_eq!(filter.since, Some(since));
    assert_eq!(filter.provider.as_deref(), Some("anthropic"));
    assert_eq!(filter.model.as_deref(), Some("claude-sonnet-5"));
    assert_eq!(filter.ids.as_deref(), Some(&["a".to_owned()][..]));
}

#[test]
fn canonical_prompt_carries_request_identity() {
    let request = SampledRequest {
        ai_request_id: AiRequestId::new("req-1"),
        context_id: ContextId::new_unchecked("00000000-0000-0000-0000-00000000c0de"),
        provider: "anthropic".to_owned(),
        model: "claude-sonnet-5".to_owned(),
        system_prompt_override: Some("be terse".to_owned()),
        messages: vec![CanonicalMessage {
            role: "user".to_owned(),
            content: "hi".to_owned(),
        }],
        response_text: Some("hello".to_owned()),
        offered_tools: Some(serde_json::json!([{"name": "search"}])),
        prepared_body_sha256: Some("abc".to_owned()),
        latency_ms: Some(10),
        cost_microdollars: 5,
        created_at: Utc::now(),
    };
    let prompt = request.canonical_prompt();
    assert_eq!(prompt.provider, "anthropic");
    assert_eq!(prompt.model, "claude-sonnet-5");
    assert_eq!(prompt.system_prompt.as_deref(), Some("be terse"));
    assert_eq!(prompt.messages.len(), 1);
    assert!(prompt.offered_tools.is_some());
}
