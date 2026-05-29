//! Tests for AiRequestStats, ProviderStatsRow, ModelStatsRow —
//! aggregate AI cost/performance rollup types not covered elsewhere.

use systemprompt_logging::trace::{AiRequestStats, ModelStatsRow, ProviderStatsRow};

#[test]
fn ai_request_stats_default_is_zero() {
    let s = AiRequestStats::default();
    assert_eq!(s.total_requests, 0);
    assert_eq!(s.total_input_tokens, 0);
    assert_eq!(s.total_output_tokens, 0);
    assert_eq!(s.total_cost_microdollars, 0);
    assert_eq!(s.avg_latency_ms, 0);
    assert!(s.by_provider.is_empty());
    assert!(s.by_model.is_empty());
}

#[test]
fn ai_request_stats_construction() {
    let s = AiRequestStats {
        total_requests: 100,
        total_input_tokens: 50_000,
        total_output_tokens: 20_000,
        total_cost_microdollars: 1_200,
        avg_latency_ms: 350,
        by_provider: vec![],
        by_model: vec![],
    };
    assert_eq!(s.total_requests, 100);
    assert_eq!(s.avg_latency_ms, 350);
}

#[test]
fn ai_request_stats_with_provider_rows() {
    let row = ProviderStatsRow {
        provider: "anthropic".to_owned(),
        request_count: 50,
        total_tokens: 70_000,
        total_cost_microdollars: 800,
        avg_latency_ms: 400,
    };
    let s = AiRequestStats {
        total_requests: 50,
        total_input_tokens: 40_000,
        total_output_tokens: 30_000,
        total_cost_microdollars: 800,
        avg_latency_ms: 400,
        by_provider: vec![row],
        by_model: vec![],
    };
    assert_eq!(s.by_provider.len(), 1);
    assert_eq!(s.by_provider[0].provider, "anthropic");
    assert_eq!(s.by_provider[0].request_count, 50);
}

#[test]
fn ai_request_stats_with_model_rows() {
    let row = ModelStatsRow {
        model: "claude-opus-4-7".to_owned(),
        provider: "anthropic".to_owned(),
        request_count: 30,
        total_tokens: 45_000,
        total_cost_microdollars: 600,
        avg_latency_ms: 500,
    };
    let s = AiRequestStats {
        total_requests: 30,
        total_input_tokens: 25_000,
        total_output_tokens: 20_000,
        total_cost_microdollars: 600,
        avg_latency_ms: 500,
        by_provider: vec![],
        by_model: vec![row],
    };
    assert_eq!(s.by_model.len(), 1);
    assert_eq!(s.by_model[0].model, "claude-opus-4-7");
    assert_eq!(s.by_model[0].provider, "anthropic");
}

#[test]
fn ai_request_stats_debug_and_clone() {
    let s = AiRequestStats::default();
    let cloned = s.clone();
    assert_eq!(cloned.total_requests, s.total_requests);
    assert!(format!("{s:?}").contains("AiRequestStats"));
}

#[test]
fn ai_request_stats_serialize_roundtrip() {
    let s = AiRequestStats {
        total_requests: 5,
        total_input_tokens: 1000,
        total_output_tokens: 500,
        total_cost_microdollars: 10,
        avg_latency_ms: 200,
        by_provider: vec![ProviderStatsRow {
            provider: "openai".to_owned(),
            request_count: 5,
            total_tokens: 1500,
            total_cost_microdollars: 10,
            avg_latency_ms: 200,
        }],
        by_model: vec![ModelStatsRow {
            model: "gpt-4o".to_owned(),
            provider: "openai".to_owned(),
            request_count: 5,
            total_tokens: 1500,
            total_cost_microdollars: 10,
            avg_latency_ms: 200,
        }],
    };
    let json = serde_json::to_string(&s).unwrap();
    let back: AiRequestStats = serde_json::from_str(&json).unwrap();
    assert_eq!(back.total_requests, 5);
    assert_eq!(back.by_provider.len(), 1);
    assert_eq!(back.by_model.len(), 1);
    assert_eq!(back.by_model[0].model, "gpt-4o");
}

#[test]
fn provider_stats_row_debug_and_clone() {
    let r = ProviderStatsRow {
        provider: "anthropic".to_owned(),
        request_count: 10,
        total_tokens: 20_000,
        total_cost_microdollars: 300,
        avg_latency_ms: 250,
    };
    let cloned = r.clone();
    assert_eq!(cloned.provider, r.provider);
    assert!(format!("{r:?}").contains("ProviderStatsRow"));
}

#[test]
fn model_stats_row_debug_and_clone() {
    let r = ModelStatsRow {
        model: "claude-haiku".to_owned(),
        provider: "anthropic".to_owned(),
        request_count: 8,
        total_tokens: 12_000,
        total_cost_microdollars: 150,
        avg_latency_ms: 120,
    };
    let cloned = r.clone();
    assert_eq!(cloned.model, r.model);
    assert!(format!("{r:?}").contains("ModelStatsRow"));
}
