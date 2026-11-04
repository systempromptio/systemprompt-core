//! Behaviour tests for the provider-neutral embedding types: builder
//! defaults and typed ids, the empty-provider hint, serde skips, the
//! estimated/reported usage constructors and the dimension probe.

use systemprompt_identifiers::{ModelId, ProviderId};
use systemprompt_models::ai::{EmbeddingRequest, EmbeddingResponse, EmbeddingUsage};

#[test]
fn request_builder_sets_dimensions_and_provider() {
    let request = EmbeddingRequest::new("text-embedding-005", vec!["a".to_owned()])
        .with_dimensions(256)
        .with_provider("gemini");
    assert_eq!(request.model, ModelId::new("text-embedding-005"));
    assert_eq!(request.dimensions, Some(256));
    assert_eq!(request.provider(), Some(&ProviderId::new("gemini")));
}

#[test]
fn an_empty_provider_hint_reads_as_none() {
    let request = EmbeddingRequest::new("m", Vec::new()).with_provider("");
    assert_eq!(request.provider(), None);
    assert_eq!(request.dimensions, None);
}

#[test]
fn with_inputs_keeps_everything_but_the_texts() {
    let request = EmbeddingRequest::new("m", vec!["x".to_owned()])
        .with_dimensions(8)
        .with_provider("openai");
    let chunk = request.with_inputs(vec!["y".to_owned(), "z".to_owned()]);
    assert_eq!(chunk.model, request.model);
    assert_eq!(chunk.dimensions, Some(8));
    assert_eq!(chunk.provider(), request.provider());
    assert_eq!(chunk.inputs, vec!["y", "z"]);
}

#[test]
fn request_serde_skips_unset_optionals() {
    let request = EmbeddingRequest::new("m", vec!["x".to_owned()]);
    let json = serde_json::to_value(&request).expect("serializes");
    assert_eq!(json, serde_json::json!({ "model": "m", "inputs": ["x"] }));
    let back: EmbeddingRequest = serde_json::from_value(json).expect("deserializes");
    assert_eq!(back, request);
}

#[test]
fn usage_constructors_and_sum_carry_the_estimate_flag() {
    let reported = EmbeddingUsage::reported(4, 4);
    let estimated = EmbeddingUsage::from_estimate(3);
    assert!(!reported.estimated);
    assert!(estimated.estimated);
    assert_eq!(estimated.total_tokens, 3);
    let sum = reported.summed_with(estimated);
    assert_eq!(sum.input_tokens, 7);
    assert_eq!(sum.total_tokens, 7);
    assert!(sum.estimated);
    assert_eq!(
        reported.summed_with(reported),
        EmbeddingUsage::reported(8, 8)
    );
    let parsed: EmbeddingUsage =
        serde_json::from_value(serde_json::json!({ "input_tokens": 1, "total_tokens": 1 }))
            .expect("estimated defaults to false");
    assert_eq!(parsed, EmbeddingUsage::reported(1, 1));
}

#[test]
fn response_reports_the_vector_width() {
    let response = EmbeddingResponse {
        model: ModelId::new("m"),
        vectors: vec![vec![0.5, 0.25], vec![1.0, 0.0]],
        usage: EmbeddingUsage::reported(2, 2),
    };
    assert_eq!(response.dimensions(), Some(2));
    let empty = EmbeddingResponse::empty(ModelId::new("m"));
    assert_eq!(empty.dimensions(), None);
    assert_eq!(empty.usage, EmbeddingUsage::default());
    let parsed: EmbeddingResponse =
        serde_json::from_value(serde_json::json!({ "model": "m", "vectors": [[1.0]] }))
            .expect("usage defaults");
    assert_eq!(parsed.usage, EmbeddingUsage::default());
}
