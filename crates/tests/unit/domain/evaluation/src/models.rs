use chrono::Utc;
use systemprompt_evaluation::CanonicalPrompt;
use systemprompt_identifiers::{AiRequestId, ContextId, ModelId, ProviderId};
use systemprompt_traits::{TraceMessage, TraceSample, TraceSampleFilter};

#[test]
fn sample_filter_builder_sets_fields() {
    let since = Utc::now();
    let filter = TraceSampleFilter::with_limit(7)
        .since(since)
        .provider(ProviderId::new("anthropic"))
        .model(ModelId::new("claude-sonnet-5"))
        .ids(vec![AiRequestId::new("a")]);
    assert_eq!(filter.limit, 7);
    assert_eq!(filter.since, Some(since));
    assert_eq!(
        filter.provider.as_ref().map(ProviderId::as_str),
        Some("anthropic")
    );
    assert_eq!(
        filter.model.as_ref().map(ModelId::as_str),
        Some("claude-sonnet-5")
    );
    assert_eq!(filter.ids.as_deref(), Some(&[AiRequestId::new("a")][..]));
}

#[test]
fn canonical_prompt_carries_request_identity() {
    let sample = TraceSample {
        ai_request_id: AiRequestId::new("req-1"),
        context_id: ContextId::try_new("00000000-0000-0000-0000-00000000c0de")
            .expect("valid ContextId"),
        provider: ProviderId::new("anthropic"),
        model: ModelId::new("claude-sonnet-5"),
        system_prompt_override: Some("be terse".to_owned()),
        messages: vec![TraceMessage {
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
    let prompt = CanonicalPrompt::from_sample(&sample);
    assert_eq!(prompt.provider.as_str(), "anthropic");
    assert_eq!(prompt.model.as_str(), "claude-sonnet-5");
    assert_eq!(prompt.system_prompt.as_deref(), Some("be terse"));
    assert_eq!(prompt.messages.len(), 1);
    assert!(prompt.offered_tools.is_some());
}
