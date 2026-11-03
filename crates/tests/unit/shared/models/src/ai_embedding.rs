//! Behaviour tests for the provider-neutral embedding types: builder
//! defaults, the empty-provider hint, serde skips and the dimension probe.

use systemprompt_models::ai::{EmbeddingRequest, EmbeddingResponse, EmbeddingUsage};

#[test]
fn request_builder_sets_dimensions_and_provider() {
    let request = EmbeddingRequest::new("text-embedding-005", vec!["a".to_owned()])
        .with_dimensions(256)
        .with_provider("gemini");
    assert_eq!(request.model, "text-embedding-005");
    assert_eq!(request.dimensions, Some(256));
    assert_eq!(request.provider(), Some("gemini"));
}

#[test]
fn an_empty_provider_hint_reads_as_none() {
    let request = EmbeddingRequest::new("m", Vec::new()).with_provider("");
    assert_eq!(request.provider(), None);
    assert_eq!(request.dimensions, None);
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
fn response_reports_the_vector_width() {
    let response = EmbeddingResponse {
        model: "m".to_owned(),
        vectors: vec![vec![0.5, 0.25], vec![1.0, 0.0]],
        usage: EmbeddingUsage {
            input_tokens: 2,
            total_tokens: 2,
        },
    };
    assert_eq!(response.dimensions(), Some(2));
    let empty = EmbeddingResponse {
        model: "m".to_owned(),
        vectors: Vec::new(),
        usage: EmbeddingUsage::default(),
    };
    assert_eq!(empty.dimensions(), None);
    let parsed: EmbeddingResponse =
        serde_json::from_value(serde_json::json!({ "model": "m", "vectors": [[1.0]] }))
            .expect("usage defaults");
    assert_eq!(parsed.usage, EmbeddingUsage::default());
}
