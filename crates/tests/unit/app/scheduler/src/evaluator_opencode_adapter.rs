use std::ffi::OsString;
use systemprompt_evaluation::capabilities::VerifiedNativeTarget;
use systemprompt_evaluation::experiments::execution::ExecutionLimits;
use systemprompt_evaluation::experiments::{ClientKind, FrozenCostEnvelope, FrozenSettings};
use systemprompt_identifiers::{EvalExecutionId, ModelId, SessionId};
use systemprompt_scheduler::services::evaluator::adapters::opencode::ADAPTER;
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

fn value<'a>(args: &'a [String], flag: &str) -> &'a str {
    let index = args
        .iter()
        .position(|argument| argument == flag)
        .expect("required flag");
    &args[index + 1]
}

fn target() -> VerifiedNativeTarget {
    VerifiedNativeTarget {
        client: ClientKind::Opencode,
        platform: "linux".to_owned(),
        architecture: "x86_64".to_owned(),
        client_version: "1.18.29".to_owned(),
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


fn config(args: &[String]) -> serde_json::Value {
    serde_json::from_str(&args[6]).unwrap()
}
#[test]
fn native_invocation_enforces_limits_and_purpose_permissions() {
    for purpose in [
        ClientPurpose::Execution,
        ClientPurpose::Judge,
        ClientPurpose::Suggestion,
    ] {
        let args = args(purpose, "--dangerously-skip-permissions");
        assert_eq!(args[0], "/usr/local/bin/node");
        assert_eq!(args[1], "/opt/systemprompt/opencode-runner.cjs");
        assert_eq!(args[3], "4096");
        assert_eq!(args[4], "claude-opus-5");
        assert_eq!(
            args[5],
            if purpose == ClientPurpose::Execution {
                "execution"
            } else {
                "review"
            }
        );
        assert!(args.iter().any(|arg| arg == "--pure"));
        assert_eq!(value(&args, "--format"), "json");
        assert_eq!(value(&args, "--model"), "systemprompt/claude-opus-5");
        assert_eq!(value(&args, "--agent"), "evaluation");
        assert_eq!(
            &args[args.len() - 2..],
            ["--", "--dangerously-skip-permissions"]
        );
        let cfg = config(&args);
        for auxiliary in ["title", "summary", "compaction"] {
            assert_eq!(cfg["agent"][auxiliary]["disable"], true);
        }
        assert_eq!(cfg["permission"]["*"], "deny");
        assert_eq!(cfg["permission"]["read"]["../*"], "deny");
        if purpose == ClientPurpose::Execution {
            assert_eq!(cfg["permission"]["edit"]["../*"], "deny");
        }
        assert_eq!(
            cfg["provider"]["systemprompt"]["models"]["claude-opus-5"]["limit"]["output"],
            4096
        );
        if purpose != ClientPurpose::Execution {
            assert_eq!(cfg["permission"]["edit"], "deny");
            assert_eq!(cfg["agent"]["evaluation"]["permission"]["edit"], "deny");
            assert_eq!(cfg["permission"]["skill"], "deny");
            assert_eq!(
                cfg["permission"]["evaluation_fixture_evaluation_fixture"],
                "deny"
            );
        }
    }
    let limits = ExecutionLimits {
        max_turns: 1,
        max_output_tokens: 512,
        ..ExecutionLimits::default()
    };
    let model = ModelId::new("gpt-5");
    let invocation = AdapterInvocation {
        model: &model,
        limits: &limits,
        purpose: ClientPurpose::Execution,
        prompt: "case",
    };
    let bounded = strings(ADAPTER.arguments(&invocation).unwrap());
    assert_eq!(&bounded[2..4], ["1", "512"]);
    let cfg = config(&bounded);
    assert_eq!(cfg["agent"]["evaluation"]["steps"], 1);
    assert_eq!(
        cfg["provider"]["systemprompt"]["models"]["gpt-5"]["limit"]["output"],
        512
    );
    assert!(
        ADAPTER
            .arguments(&AdapterInvocation {
                prompt: &"x".repeat(65_537),
                ..invocation
            })
            .is_err()
    );
    assert!(
        ADAPTER
            .arguments(&AdapterInvocation {
                prompt: "bad\0prompt",
                ..invocation
            })
            .is_err()
    );
}
#[test]
fn configuration_is_private_reproducible_and_exclusively_gateway_scoped() {
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
    assert_eq!(ADAPTER.skill_directory(), ".config/opencode/skills");
    assert!(archive.files.values().all(|f| !f.executable));
    assert_eq!(
        archive.files["home/.local/share/opencode/auth.json"].bytes,
        b"{}"
    );
    let env = std::str::from_utf8(&archive.files["client.env"].bytes).unwrap();
    for flag in [
        "OPENCODE_DISABLE_PROJECT_CONFIG=1",
        "OPENCODE_DISABLE_AUTOUPDATE=1",
        "OPENCODE_DISABLE_MODELS_FETCH=1",
    ] {
        assert!(env.contains(flag));
    }
    let cfg: serde_json::Value =
        serde_json::from_slice(&archive.files["home/.config/opencode/opencode.json"].bytes)
            .unwrap();
    assert_eq!(
        cfg["enabled_providers"],
        serde_json::json!(["systemprompt"])
    );
    assert_eq!(
        cfg["provider"]["systemprompt"]["options"]["baseURL"],
        "http://eval-relay-fixture:8090/v1"
    );
    assert_eq!(
        cfg["provider"]["systemprompt"]["options"]["apiKey"],
        "spexec_fixture.signature"
    );
    assert_eq!(cfg["mcp"]["evaluation_fixture"]["oauth"], false);
    assert_eq!(cfg["share"], "disabled");
    context.relay_url = "https://api.openai.com";
    assert!(ADAPTER.configuration(&context).is_err());
    context.relay_url = "http://eval-relay-fixture:8090";
    context.execution_token = "bad\nENV=injected";
    assert!(ADAPTER.configuration(&context).is_err());
}
#[test]
fn release_parser_and_configuration_reject_unknown_native_versions() {
    assert_eq!(ADAPTER.parse_version(b"1.18.29\n").unwrap(), "1.18.29");
    for version in [
        "v1.18.29",
        "1.18.29-dev",
        "01.18.29",
        "1.18",
        "1.18.29\n0.5.1",
    ] {
        assert!(ADAPTER.parse_version(version.as_bytes()).is_err());
    }
    let mut target = target();
    target.client_version = "0.5.1".to_owned();
    let frozen = frozen();
    assert!(
        ADAPTER
            .configuration(&AdapterContext {
                relay_url: "http://eval-relay-fixture:8090",
                execution_token: "token",
                session_id: &SessionId::generate(),
                execution_id: &EvalExecutionId::generate(),
                target: &target,
                frozen: &frozen
            })
            .is_err()
    );
}
fn event(kind: &str, id: &str, fields: serde_json::Value) -> serde_json::Value {
    let ty = match kind {
        "step_finish" => "step-finish",
        "step_start" => "step-start",
        "tool_use" => "tool",
        other => other,
    };
    let mut part = serde_json::json!({"id":id,"sessionID":"s1","type":ty});
    part.as_object_mut()
        .unwrap()
        .extend(fields.as_object().unwrap().clone());
    serde_json::json!({"type":kind,"sessionID":"s1","part":part})
}
fn stream(events: &[serde_json::Value]) -> Vec<u8> {
    let mut events = events.to_vec();
    if !events.iter().any(|event| {
        event["type"]
            .as_str()
            .is_some_and(|kind| kind.starts_with("opencode.runner_"))
    }) {
        events.push(serde_json::json!({"type":"opencode.runner_result","process_exit_code":0,"signal":null,"provider_attempts":2,"max_requests":12,"output_limit_reached":false}));
    }
    raw_stream(&events)
}
fn raw_stream(events: &[serde_json::Value]) -> Vec<u8> {
    events
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
        .into_bytes()
}
#[test]
fn independent_steps_deduplicate_usage_and_preserve_unknown_denominators() {
    let first = event(
        "step_finish",
        "p1",
        serde_json::json!({"reason":"tool-calls","tokens":{"input":10,"output":4}}),
    );
    let second = event(
        "step_finish",
        "p2",
        serde_json::json!({"reason":"stop","tokens":{"input":6,"output":2}}),
    );
    let tool = event(
        "tool_use",
        "t1",
        serde_json::json!({"tool":"read","state":{"status":"completed"}}),
    );
    let text = event("text", "txt1", serde_json::json!({"text":"final answer"}));
    let output = ADAPTER
        .normalize(&stream(&[
            first.clone(),
            first.clone(),
            tool.clone(),
            tool,
            text.clone(),
            text,
            second.clone(),
        ]))
        .unwrap();
    assert_eq!(output.completion, NativeCompletion::Completed);
    assert_eq!(output.text, "final answer");
    assert_eq!(output.reported_input_tokens, Some(16));
    assert_eq!(output.reported_output_tokens, Some(6));
    assert_eq!(output.tool_calls, ["read"]);
    let unknown = event("step_finish", "p3", serde_json::json!({"reason":"stop"}));
    let output = ADAPTER
        .normalize(&stream(&[first, unknown, second]))
        .unwrap();
    assert_eq!(output.reported_input_tokens, None);
    assert_eq!(output.reported_output_tokens, None);
}
#[test]
fn failure_and_unfinished_steps_cannot_produce_completed_judgments() {
    let stop = event(
        "step_finish",
        "p1",
        serde_json::json!({"reason":"stop","tokens":{"input":3,"output":1}}),
    );
    let start = event("step_start", "p2", serde_json::json!({}));
    assert_eq!(
        ADAPTER
            .normalize(&stream(&[stop.clone(), start]))
            .unwrap()
            .completion,
        NativeCompletion::Incomplete
    );
    let error = serde_json::json!({"type":"error","sessionID":"s1","error":{"name":"APIError"}});
    let output = ADAPTER.normalize(&stream(&[error, stop])).unwrap();
    assert_eq!(output.completion, NativeCompletion::Failed);
    assert_eq!(output.reported_input_tokens, Some(3));
    let evidence = normalize_evidence(&ADAPTER, b"not JSON");
    assert!(evidence.diagnostic.is_some());
    assert_eq!(evidence.output.completion, NativeCompletion::Incomplete);
}
#[test]
fn mixed_sessions_conflicting_parts_and_invalid_usage_are_rejected() {
    let one = event("text", "p1", serde_json::json!({"text":"one"}));
    let two = event("text", "p1", serde_json::json!({"text":"two"}));
    assert!(ADAPTER.normalize(&stream(&[one.clone(), two])).is_err());
    let mut other = one.clone();
    other["sessionID"] = serde_json::json!("s2");
    assert!(ADAPTER.normalize(&stream(&[one, other])).is_err());
    let negative = event(
        "step_finish",
        "p1",
        serde_json::json!({"reason":"stop","tokens":{"input":-1}}),
    );
    assert!(ADAPTER.normalize(&stream(&[negative])).is_err());
    assert!(
        ADAPTER
            .normalize(&vec![b'x'; 16 * 1024 * 1024 + 1])
            .is_err()
    );
}


#[test]
fn runner_bound_failure_preserves_native_usage_and_cannot_become_success() {
    let completed = event(
        "step_finish",
        "p1",
        serde_json::json!({"reason":"stop","tokens":{"input":11,"output":7}}),
    );
    let result = serde_json::json!({"type":"opencode.runner_result","process_exit_code":0,"signal":null,"provider_attempts":2,"max_requests":1,"output_limit_reached":false});
    let output = ADAPTER
        .normalize(&stream(&[completed.clone(), result.clone()]))
        .unwrap();
    assert_eq!(output.completion, NativeCompletion::Failed);
    assert_eq!(output.reported_input_tokens, Some(11));
    assert_eq!(output.reported_output_tokens, Some(7));
    assert!(
        ADAPTER
            .normalize(&stream(&[result.clone(), completed.clone()]))
            .is_err()
    );
    assert!(
        ADAPTER
            .normalize(&stream(&[result.clone(), result.clone()]))
            .is_err()
    );
    let mut malformed = result.clone();
    malformed["process_exit_code"] = serde_json::json!("zero");
    assert!(
        ADAPTER
            .normalize(&stream(&[completed.clone(), malformed]))
            .is_err()
    );
    let mut clean = result.clone();
    clean["provider_attempts"] = serde_json::json!(1);
    assert_eq!(
        ADAPTER
            .normalize(&stream(&[completed.clone(), clean.clone()]))
            .unwrap()
            .completion,
        NativeCompletion::Completed
    );
    clean["output_limit_reached"] = serde_json::json!(true);
    assert_eq!(
        ADAPTER
            .normalize(&stream(&[completed, clean]))
            .unwrap()
            .completion,
        NativeCompletion::Failed
    );
}


#[test]
fn missing_runner_termination_never_completes_v2_but_retains_native_usage() {
    let completed = event(
        "step_finish",
        "p1",
        serde_json::json!({"reason":"stop","tokens":{"input":11,"output":7}}),
    );
    let text = event(
        "text",
        "t1",
        serde_json::json!({"text":"retained native answer"}),
    );
    let missing = ADAPTER
        .normalize(&raw_stream(&[text.clone(), completed.clone()]))
        .unwrap();
    assert_eq!(missing.completion, NativeCompletion::Incomplete);
    assert_eq!(missing.text, "retained native answer");
    assert_eq!(missing.reported_input_tokens, Some(11));
    assert_eq!(missing.reported_output_tokens, Some(7));
    let error = serde_json::json!({"type":"error","sessionID":"s1","error":{"name":"APIError"}});
    let failed = ADAPTER
        .normalize(&raw_stream(&[completed.clone(), error]))
        .unwrap();
    assert_eq!(failed.completion, NativeCompletion::Failed);
    assert_eq!(failed.reported_input_tokens, Some(11));
    assert_eq!(failed.reported_output_tokens, Some(7));
    let terminated = ADAPTER.normalize(&stream(&[text, completed])).unwrap();
    assert_eq!(terminated.completion, NativeCompletion::Completed);
}
