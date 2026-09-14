use std::ffi::OsString;
use systemprompt_evaluation::capabilities::VerifiedNativeTarget;
use systemprompt_evaluation::experiments::execution::ExecutionLimits;
use systemprompt_evaluation::experiments::{ClientKind, FrozenCostEnvelope, FrozenSettings};
use systemprompt_identifiers::{EvalExecutionId, ModelId, SessionId};
use systemprompt_scheduler::services::evaluator::adapters::codex::ADAPTER;
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
        client: ClientKind::Codex,
        platform: "linux".to_owned(),
        architecture: "x86_64".to_owned(),
        client_version: "0.154.0".to_owned(),
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
fn invocation_pins_native_runner_and_enforces_purpose_specific_permissions() {
    let execution = args(
        ClientPurpose::Execution,
        "--dangerously-bypass-approvals-and-sandbox",
    );
    assert_eq!(execution[0], "/usr/local/bin/node");
    assert_eq!(execution[1], "/opt/systemprompt/codex-runner.cjs");
    assert_eq!(
        &execution[2..7],
        ["12", "4096", "claude-opus-5", "execution", "--"]
    );
    assert_eq!(value(&execution, "--model"), "claude-opus-5");
    assert_eq!(
        &execution[execution.len() - 2..],
        ["--", "--dangerously-bypass-approvals-and-sandbox"]
    );
    for setting in [
        "approval_policy=\"never\"",
        "mcp_servers.evaluation_fixture.tools.evaluation_fixture.approval_mode=\"approve\"",
        "permissions.evaluation.network.enabled=false",
        "shell_environment_policy.inherit=\"none\"",
        "features.hooks=false",
        "features.apps=false",
        "features.plugins=false",
        "features.multi_agent=false",
        "features.view_image=false",
        "features.image_generation=false",
        "web_search=\"disabled\"",
    ] {
        assert!(
            execution.iter().any(|arg| arg == setting),
            "missing {setting}"
        );
    }
    let filesystem = execution
        .iter()
        .find(|arg| arg.starts_with("permissions.evaluation.filesystem="))
        .unwrap();
    assert!(filesystem.contains(r#""/proc"="deny""#));
    assert!(filesystem.contains(r#""/home/tester/work"="write""#));
    assert!(filesystem.contains(r#""/home/tester/.agents/skills"="read""#));
    for purpose in [ClientPurpose::Judge, ClientPurpose::Suggestion] {
        let argv = args(purpose, "evaluate");
        assert_eq!(argv[5], "review");
        let filesystem = argv
            .iter()
            .find(|arg| arg.starts_with("permissions.evaluation.filesystem="))
            .unwrap();
        assert!(filesystem.contains(r#""/home/tester/work"="read""#));
        assert!(filesystem.contains(r#""/home/tester/.agents/skills"="deny""#));
        for setting in ["mcp_servers.evaluation_fixture.enabled=false"] {
            assert!(argv.iter().any(|arg| arg == setting));
        }
    }
}
#[test]
fn requested_limits_reach_runner_without_prompt_or_model_argument_injection() {
    let model = ModelId::new("gpt-5");
    let limits = ExecutionLimits {
        max_turns: 1,
        max_output_tokens: 256,
        ..ExecutionLimits::default()
    };
    let input = AdapterInvocation {
        model: &model,
        limits: &limits,
        purpose: ClientPurpose::Execution,
        prompt: "case",
    };
    let argv = strings(ADAPTER.arguments(&input).unwrap());
    assert_eq!(&argv[2..4], ["1", "256"]);
    for prompt in ["x".repeat(65_537), "bad\0prompt".to_owned()] {
        assert!(
            ADAPTER
                .arguments(&AdapterInvocation {
                    prompt: &prompt,
                    ..input
                })
                .is_err()
        );
    }
    let model = ModelId::new("gpt-5\nTOKEN=secret");
    assert!(
        ADAPTER
            .arguments(&AdapterInvocation {
                model: &model,
                ..input
            })
            .is_err()
    );
}
#[test]
fn isolated_configuration_only_exposes_scoped_credentials_to_runner() {
    let target = target();
    let mut frozen = frozen();
    frozen.fixture_timezone = "Asia/Tokyo".to_owned();
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
    assert_eq!(ADAPTER.skill_directory(), ".agents/skills");
    assert_eq!(ADAPTER.executable(), "/opt/systemprompt/codex/bin/codex");
    assert!(archive.files.values().all(|f| !f.executable));
    assert!(archive.files["home/.codex/config.toml"].bytes.is_empty());
    let env = std::str::from_utf8(&archive.files["client.env"].bytes).unwrap();
    assert!(env.contains("CODEX_EVALUATION_TOKEN=spexec_fixture.signature\n"));
    assert!(!env.contains("OPENAI_API_KEY="));
    assert!(env.contains("TZ=Asia/Tokyo\n"));
    assert!(!format!("{context:?}").contains("spexec_fixture.signature"));
    context.relay_url = "https://api.openai.com";
    assert!(ADAPTER.configuration(&context).is_err());
    context.relay_url = "http://eval-relay-fixture:8090";
    context.execution_token = "token\nX=bad";
    assert!(ADAPTER.configuration(&context).is_err());
}
#[test]
fn codex_version_parser_requires_native_release_identity() {
    assert_eq!(
        ADAPTER.parse_version(b"codex-cli 0.154.0\n").unwrap(),
        "0.154.0"
    );
    for version in [
        "0.154.0",
        "codex-cli 0.154.0-dev",
        "codex-cli 00.154.0",
        "codex-cli 0.154",
        "codex-cli 0.154.0\nextra",
    ] {
        assert!(ADAPTER.parse_version(version.as_bytes()).is_err());
    }
}
#[test]
fn codex_items_deduplicate_and_terminal_usage_is_advisory() {
    let item = r#"{"type":"item.completed","item":{"id":"i1","type":"command_execution","command":"ls","status":"completed","exit_code":0}}"#;
    let text =
        r#"{"type":"item.completed","item":{"id":"i2","type":"agent_message","text":"answer"}}"#;
    let terminal = r#"{"type":"turn.completed","usage":{"input_tokens":10,"cached_input_tokens":8,"output_tokens":3}}"#;
    let output = ADAPTER
        .normalize(format!("{item}\n{item}\n{text}\n{terminal}\n{terminal}").as_bytes())
        .unwrap();
    assert_eq!(output.text, "answer");
    assert_eq!(output.completion, NativeCompletion::Completed);
    assert_eq!(output.tool_calls, ["exec_command"]);
    assert_eq!(output.reported_input_tokens, Some(10));
    assert_eq!(output.reported_output_tokens, Some(3));
    let unknown = ADAPTER.normalize(br#"{"type":"turn.completed"}"#).unwrap();
    assert_eq!(unknown.reported_input_tokens, None);
}
#[test]
fn incomplete_failed_and_conflicting_codex_evidence_never_yields_success() {
    let unfinished = br#"{"type":"item.started","item":{"id":"i1","type":"command_execution"}}
{"type":"turn.completed","usage":{"input_tokens":5}}"#;
    assert_eq!(
        ADAPTER.normalize(unfinished).unwrap().completion,
        NativeCompletion::Incomplete
    );
    let failed = ADAPTER
        .normalize(
            br#"{"type":"turn.failed","error":{"message":"limit"},"usage":{"input_tokens":5}}"#,
        )
        .unwrap();
    assert_eq!(failed.completion, NativeCompletion::Failed);
    assert_eq!(failed.reported_input_tokens, Some(5));
    for malformed in [
        "{\"type\":\"turn.completed\",\"usage\":{\"input_tokens\":-1}}",
        "{\"type\":\"thread.started\",\"thread_id\":\"a\"}\n{\"type\":\"thread.started\",\"thread_id\":\"b\"}",
        "{\"type\":\"turn.completed\"}\n{\"type\":\"turn.failed\"}",
        "{\"type\":\"item.completed\",\"item\":{\"id\":\"i1\",\"type\":\"web_search\"}}",
    ] {
        assert!(ADAPTER.normalize(malformed.as_bytes()).is_err());
    }
    let evidence = normalize_evidence(&ADAPTER, b"not JSON");
    assert!(evidence.diagnostic.is_some());
    assert_eq!(evidence.output.completion, NativeCompletion::Incomplete);
}

#[test]
fn pinned_codex_uses_registered_feature_keys_for_image_tool_isolation() {
    // Codex0.154.0 `features list` exposes view_image/image_generation; its
    // strict config rejects the legacy tools.view_image key before startup.
    for purpose in [
        ClientPurpose::Execution,
        ClientPurpose::Judge,
        ClientPurpose::Suggestion,
    ] {
        let argv = args(purpose, "inspect retained evidence");
        assert!(argv.iter().any(|arg| arg == "--strict-config"));
        assert!(argv.iter().any(|arg| arg == "features.view_image=false"));
        assert!(
            argv.iter()
                .any(|arg| arg == "features.image_generation=false")
        );
        assert!(!argv.iter().any(|arg| arg.starts_with("tools.view_image=")));
        assert!(
            argv.iter()
                .any(|arg| arg == "permissions.evaluation.network.enabled=false")
        );
        assert!(
            argv.iter()
                .any(|arg| arg == "shell_environment_policy.inherit=\"none\"")
        );
    }
}
