use systemprompt_brain::model::{ModelOutputError, kind_schema, parse_output, request_for};
use systemprompt_models::brain::{BrainModelResponse, FactSource};

use crate::pipeline_fixtures::{kind_decl, source_text, span_source};

fn response(structured: Option<serde_json::Value>, text: &str) -> BrainModelResponse {
    BrainModelResponse {
        model: "test-model".to_owned(),
        text: text.to_owned(),
        structured,
        input_tokens: 10,
        output_tokens: 5,
        ai_request_id: None,
    }
}

fn parse(output: serde_json::Value) -> Result<usize, ModelOutputError> {
    parse_output(
        &kind_decl(),
        &response(Some(output), ""),
        &span_source(None),
        "core.model",
    )
    .map(|c| c.len())
}

#[test]
fn the_schema_is_strict_and_requires_every_key() {
    let schema = kind_schema(&kind_decl()).schema;
    assert_eq!(schema["additionalProperties"], serde_json::json!(false));
    assert_eq!(schema["required"], serde_json::json!(["facts"]));
    let item = &schema["properties"]["facts"]["items"];
    assert_eq!(item["additionalProperties"], serde_json::json!(false));
    let required: Vec<&str> = item["required"]
        .as_array()
        .expect("required")
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    for key in [
        "subject",
        "confidence",
        "span",
        "owner",
        "status",
        "due",
        "amount",
        "partner",
    ] {
        assert!(required.contains(&key), "{key} must be required");
    }
    assert_eq!(
        item["properties"]["owner"]["type"],
        serde_json::json!("string")
    );
    assert_eq!(
        item["properties"]["owner"]["maxLength"],
        serde_json::json!(10)
    );
    assert_eq!(
        item["properties"]["amount"]["type"],
        serde_json::json!(["number", "null"])
    );
    assert_eq!(
        item["properties"]["status"]["enum"],
        serde_json::json!(["open", "closed", null])
    );
}

#[test]
fn a_valid_answer_parses_into_candidates() {
    let output = serde_json::json!({ "facts": [{
        "subject": "client:acme", "confidence": 0.9,
        "span": { "node_id": "n1", "hash8": "abcd1234", "find": "owes", "nth": 0 },
        "owner": "billing", "status": null, "due": null, "amount": null, "partner": null
    }]});
    let candidates = parse_output(
        &kind_decl(),
        &response(Some(output), ""),
        &span_source(None),
        "core.model",
    )
    .expect("parse");
    assert_eq!(candidates.len(), 1);
    let candidate = &candidates[0];
    assert_eq!(candidate.subject.to_string(), "client:acme");
    assert_eq!(candidate.fields.len(), 1);
    assert!(matches!(
        &candidate.source,
        FactSource::Document { span: Some(_), .. }
    ));
}

#[test]
fn the_text_is_parsed_when_no_structured_output_came_back() {
    let text = r#"{"facts":[{"subject":"acme","owner":"billing"}]}"#;
    let parsed = parse_output(
        &kind_decl(),
        &response(None, text),
        &span_source(None),
        "core.model",
    );
    assert_eq!(parsed.map(|c| c.len()), Ok(1));
    let garbage = parse_output(
        &kind_decl(),
        &response(None, "sure! here you go"),
        &span_source(None),
        "core.model",
    );
    assert_eq!(garbage, Err(ModelOutputError::NotJson));
}

#[test]
fn extra_keys_are_refused() {
    let extra_fact_key =
        serde_json::json!({ "facts": [{ "subject": "acme", "owner": "b", "invented": 1 }] });
    assert!(matches!(
        parse(extra_fact_key),
        Err(ModelOutputError::Schema(_))
    ));
    let extra_root_key = serde_json::json!({ "facts": [], "notes": "x" });
    assert!(matches!(
        parse(extra_root_key),
        Err(ModelOutputError::Schema(_))
    ));
    let no_subject = serde_json::json!({ "facts": [{ "owner": "b" }] });
    assert!(matches!(
        parse(no_subject),
        Err(ModelOutputError::Schema(_))
    ));
    assert_eq!(parse(serde_json::json!({ "facts": [] })), Ok(0));
}

#[test]
fn a_request_needs_a_versioned_model_extractor() {
    let request = request_for(&kind_decl(), &source_text(), &[], 512).expect("request");
    assert_eq!(request.prompt_version, "v3");
    assert_eq!(request.max_output_tokens, 512);
    assert_eq!(request.system, "Find commitments.");
    assert!(request.user.contains("[n1 abcd1234]"));
    let mut parser = kind_decl();
    parser.extractor = None;
    assert!(request_for(&parser, &source_text(), &[], 512).is_none());
}
