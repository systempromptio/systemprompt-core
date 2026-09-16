use std::ffi::OsString;
use systemprompt_evaluation::capabilities::VerifiedNativeTarget;
use systemprompt_evaluation::experiments::execution::ExecutionLimits;
use systemprompt_evaluation::experiments::{ClientKind, FrozenCostEnvelope, FrozenSettings};
use systemprompt_identifiers::{EvalExecutionId, ModelId, SessionId};
use systemprompt_scheduler::services::evaluator::adapters::hermes::ADAPTER;
use systemprompt_scheduler::services::evaluator::adapters::{
    AdapterContext, AdapterInvocation, NativeAdapter, NativeCompletion, normalize_evidence,
};
use systemprompt_scheduler::services::evaluator::client::ClientPurpose;

fn strings(arguments: Vec<OsString>) -> Vec<String> {
    arguments
        .into_iter()
        .map(|argument| argument.into_string().expect("UTF-8 argument"))
        .collect()
}

fn args(purpose: ClientPurpose, prompt: &str) -> Vec<String> {
    strings(
        ADAPTER
            .arguments(&AdapterInvocation {
                model: &ModelId::new("claude-opus-5"),
                limits: &ExecutionLimits::default(),
                purpose,
                prompt,
            })
            .expect("bounded native invocation"),
    )
}

fn target() -> VerifiedNativeTarget {
    VerifiedNativeTarget {
        client: ClientKind::Hermes,
        platform: "linux".to_owned(),
        architecture: "x86_64".to_owned(),
        client_version: "0.21.3".to_owned(),
        adapter_version: ADAPTER.adapter_version().to_owned(),
        image_digest: "a".repeat(64),
        executable_digest: "b".repeat(64),
        native_isolation_evidence_digest: "c".repeat(64),
        native_metering_evidence_digest: "d".repeat(64),
    }
}

fn frozen() -> FrozenSettings {
    FrozenSettings {
        provider_prices_digest: "a".repeat(64),
        tool_configuration_digest: "b".repeat(64),
        fixture_clock: "2026-09-14T00:00:00Z".to_owned(),
        fixture_timezone: "UTC".to_owned(),
        permissions_digest: "c".repeat(64),
        dataset_digest: "d".repeat(64),
        rubric_digest: "e".repeat(64),
        cost_envelope: FrozenCostEnvelope {
            maximum_attempts_per_execution: 1,
            generation_microdollars_per_attempt: 1,
            judging_microdollars_per_attempt: 1,
            tool_microdollars_per_attempt: 0,
            suggestion_calls: 0,
            suggestion_microdollars_per_call: 0,
            auxiliary_calls: 0,
            auxiliary_microdollars_per_call: 0,
        },
    }
}


#[test]
fn hermes_runner_owns_exact_native_limits_and_literal_prompt() {
    let argv = args(ClientPurpose::Execution, "--yolo");
    assert_eq!(
        &argv[..6],
        [
            "/usr/local/bin/node",
            "/opt/systemprompt/hermes-runner.cjs",
            "12",
            "4096",
            "claude-opus-5",
            "execution"
        ]
    );
    assert!(argv.last().unwrap().ends_with("--yolo"));
    assert!(argv.last().unwrap().contains("/home/tester/.hermes/skills"));
    assert_eq!(args(ClientPurpose::Judge, "judge")[2], "2");
    assert_eq!(args(ClientPurpose::Suggestion, "suggest")[2], "3");
    assert_eq!(args(ClientPurpose::Judge, "judge")[5], "review");
    let model = ModelId::new("fixture");
    let limits = ExecutionLimits::default();
    assert!(
        ADAPTER
            .arguments(&AdapterInvocation {
                model: &model,
                limits: &limits,
                purpose: ClientPurpose::Execution,
                prompt: &"x".repeat(65_537)
            })
            .is_err()
    );
}
#[test]
fn hermes_configuration_disables_ambient_extensions_and_scopes_credentials() {
    let target = target();
    let frozen = frozen();
    let session = SessionId::generate();
    let execution = EvalExecutionId::generate();
    let mut context = AdapterContext {
        relay_url: "http://eval-relay-fixture:8090",
        execution_token: "spexec_fixture.signature",
        session_id: &session,
        execution_id: &execution,
        target: &target,
        frozen: &frozen,
    };
    let archive = ADAPTER.configuration(&context).unwrap();
    assert_eq!(
        archive.digest().unwrap(),
        ADAPTER.configuration(&context).unwrap().digest().unwrap()
    );
    let cfg: serde_json::Value =
        serde_json::from_slice(&archive.files["home/.hermes/evaluator-config.json"].bytes).unwrap();
    assert_eq!(cfg["skills"]["inline_shell"], false);
    assert_eq!(cfg["skills"]["project_discovery"], false);
    assert_eq!(cfg["memory"]["memory_enabled"], false);
    assert_eq!(cfg["hooks_auto_accept"], false);
    assert_eq!(cfg["auxiliary"]["title_generation"]["enabled"], false);
    assert_eq!(cfg["model"]["provider"], "custom");
    assert_eq!(cfg["model"]["base_url"], "http://127.0.0.1:8091/v1");
    assert_eq!(cfg["model"]["api_mode"], "chat_completions");
    assert!(
        cfg["model"].get("api_key").is_none(),
        "per-run loopback credentials are injected only into temporary native configuration"
    );
    assert!(cfg["fallback_providers"].as_array().unwrap().is_empty());
    assert_eq!(ADAPTER.skill_directory(), ".hermes/skills");
    let env = std::str::from_utf8(&archive.files["client.env"].bytes).unwrap();
    assert!(env.contains("HERMES_EVALUATION_TOKEN=spexec_fixture.signature\n"));
    assert!(!env.contains("OPENAI_API_KEY="));
    context.relay_url = "http://other:8090";
    assert!(ADAPTER.configuration(&context).is_err());
}
#[test]
fn hermes_version_requires_exact_release_and_unambiguous_metadata() {
    assert_eq!(ADAPTER.parse_version(b"Hermes Agent v0.21.3 (2026.9.14)\nInstall directory: /opt/hermes-source\nPython: 3.13.0\nOpenAI SDK: 2.0.0\n").unwrap(),"0.21.3");
    for value in [
        "0.21.3",
        "Hermes Agent v0.21.3 (2026.9.13)",
        "Hermes Agent v0.21.3-dev (2026.9.14)",
        "Hermes Agent v0.21.3 (2026.9.14)\nHermes Agent v0.21.4 (2026.9.14)",
    ] {
        assert!(ADAPTER.parse_version(value.as_bytes()).is_err());
    }
}
fn terminal() -> serde_json::Value {
    serde_json::json!({"type":"hermes.result","sequence":1,"process_exit_code":0,"signal":null,"output_limit_reached":false,"usage_independently_verified":false,"usage_identity_matches":true,"usage":{"completed":true,"failed":false,"input_tokens":12,"output_tokens":4},"tool_calls":["read_file"]})
}
fn stream(result: &serde_json::Value) -> Vec<u8> {
    format!("{{\"type\":\"hermes.stdout\",\"sequence\":0,\"text\":\"answer\"}}\n{result}")
        .into_bytes()
}
#[test]
fn completed_hermes_output_retains_advisory_usage_without_pricing_claims() {
    let output = ADAPTER.normalize(&stream(&terminal())).unwrap();
    assert_eq!(output.completion, NativeCompletion::Completed);
    assert_eq!(output.text, "answer");
    assert_eq!(output.reported_input_tokens, Some(12));
    assert_eq!(output.tool_calls, ["read_file"]);
    let mut result = terminal();
    result["usage"]["input_tokens"] = serde_json::Value::Null;
    assert_eq!(
        ADAPTER
            .normalize(&stream(&result))
            .unwrap()
            .reported_input_tokens,
        None
    );
}
#[test]
fn forged_success_cannot_override_native_failure_or_incomplete_usage() {
    let mut result = terminal();
    result["process_exit_code"] = serde_json::json!(1);
    let output = ADAPTER.normalize(&stream(&result)).unwrap();
    assert_eq!(output.completion, NativeCompletion::Failed);
    assert_eq!(output.reported_input_tokens, Some(12));
    let mut result = terminal();
    result["usage_identity_matches"] = serde_json::json!(false);
    assert_eq!(
        ADAPTER.normalize(&stream(&result)).unwrap().completion,
        NativeCompletion::Incomplete
    );
    let mut result = terminal();
    result["usage"] = serde_json::Value::Null;
    assert_eq!(
        ADAPTER.normalize(&stream(&result)).unwrap().completion,
        NativeCompletion::Incomplete
    );
    let evidence = normalize_evidence(&ADAPTER, b"invalid report");
    assert!(evidence.diagnostic.is_some());
    assert_eq!(evidence.output.completion, NativeCompletion::Incomplete);
}
#[test]
fn hermes_rejects_forged_verification_conflicting_chunks_and_invalid_usage() {
    let mut result = terminal();
    result["usage_independently_verified"] = serde_json::json!(true);
    assert!(ADAPTER.normalize(&stream(&result)).is_err());
    let mut result = terminal();
    result["usage"]["output_tokens"] = serde_json::json!(-1);
    assert!(ADAPTER.normalize(&stream(&result)).is_err());
    let mut result = terminal();
    result["sequence"] = serde_json::json!(3);
    assert!(ADAPTER.normalize(&stream(&result)).is_err());
    assert!(ADAPTER.normalize(b"{\"type\":\"hermes.stdout\",\"sequence\":0,\"text\":\"a\"}\n{\"type\":\"hermes.stdout\",\"sequence\":0,\"text\":\"b\"}").is_err());
}
