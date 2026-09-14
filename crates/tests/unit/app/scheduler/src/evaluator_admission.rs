use systemprompt_evaluation::experiments::ClientKind;
use systemprompt_identifiers::ModelId;
use systemprompt_scheduler::services::evaluator::adapters::NormalizedClientOutput;
use systemprompt_scheduler::services::evaluator::client::NativeClient;

#[test]
fn every_unverified_native_client_is_rejected_even_with_well_formed_caller_pins() {
    let digest = "a".repeat(64);
    for kind in [
        ClientKind::ClaudeCode,
        ClientKind::Opencode,
        ClientKind::Codex,
        ClientKind::Hermes,
    ] {
        let client = NativeClient::builder(kind, ModelId::new("fixture-model"))
            .pinned("1.2.3".to_owned(), digest.clone())
            .build()
            .expect("valid limits");
        assert!(
            client
                .admitted_target(&format!("fixture@sha256:{digest}"))
                .is_err(),
            "caller pins cannot serve as native verification evidence"
        );
        assert!(client.admitted_target("fixture:latest").is_err());
    }
}

#[test]
fn normalized_output_preserves_unknown_metering_and_bounds_tool_evidence() {
    let mut evidence = NormalizedClientOutput {
        completion:
            systemprompt_scheduler::services::evaluator::adapters::NativeCompletion::Incomplete,
        text: "fixture response".to_owned(),
        reported_input_tokens: None,
        reported_output_tokens: None,
        tool_calls: vec!["Read".to_owned()],
    };
    evidence.validate().expect("bounded output");
    let encoded = serde_json::to_value(&evidence).expect("output JSON");
    assert!(encoded["reported_input_tokens"].is_null());
    assert!(encoded["reported_output_tokens"].is_null());
    evidence.tool_calls = vec!["Read".to_owned(); 1001];
    assert!(evidence.validate().is_err());
    evidence.tool_calls = vec!["Read\nforged".to_owned()];
    assert!(evidence.validate().is_err());
}
