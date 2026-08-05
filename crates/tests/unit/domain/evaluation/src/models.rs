use std::str::FromStr;

use chrono::Utc;
use systemprompt_evaluation::{
    CanonicalMessage, EvalRunKind, JudgeVerdict, SampleFilter, SampledRequest, Verdict,
};
use systemprompt_identifiers::AiRequestId;

#[test]
fn run_kind_round_trips() {
    for kind in [EvalRunKind::Judge, EvalRunKind::Replay, EvalRunKind::Pairwise] {
        assert_eq!(EvalRunKind::from_str(kind.as_str()).expect("kind"), kind);
    }
    assert!(EvalRunKind::from_str("nope").is_err());
}

#[test]
fn verdict_round_trips() {
    for verdict in [
        Verdict::Pass,
        Verdict::Partial,
        Verdict::Fail,
        Verdict::Skipped,
    ] {
        assert_eq!(Verdict::from_str(verdict.as_str()).expect("verdict"), verdict);
    }
    assert!(Verdict::from_str("nope").is_err());
}

#[test]
fn judge_verdict_parses_minimal_payload() {
    let verdict: JudgeVerdict =
        serde_json::from_str(r#"{"overall_score": 4, "rationale": "solid"}"#).expect("parse");
    assert_eq!(verdict.overall_score, 4);
    assert!(verdict.dimension_scores.is_empty());
    assert!(verdict.repair_hint.is_none());
}

#[test]
fn judge_verdict_parses_full_payload() {
    let verdict: JudgeVerdict = serde_json::from_str(
        r#"{
            "overall_score": 2,
            "dimension_scores": [{"name": "correctness", "score": 2}],
            "rationale": "wrong flag",
            "repair_hint": "use --workspace"
        }"#,
    )
    .expect("parse");
    assert_eq!(verdict.dimension_scores.len(), 1);
    assert_eq!(verdict.repair_hint.as_deref(), Some("use --workspace"));
}

#[test]
fn judge_verdict_schema_names_required_fields() {
    let schema = JudgeVerdict::response_schema();
    let required = schema["required"].as_array().expect("required");
    assert!(required.iter().any(|v| v == "overall_score"));
    assert!(required.iter().any(|v| v == "rationale"));
    assert_eq!(schema["properties"]["overall_score"]["maximum"], 5);
}

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
