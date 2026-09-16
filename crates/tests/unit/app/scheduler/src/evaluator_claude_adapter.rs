use std::ffi::OsString;
use systemprompt_evaluation::capabilities::VerifiedNativeTarget;
use systemprompt_evaluation::experiments::execution::ExecutionLimits;
use systemprompt_evaluation::experiments::{ClientKind, FrozenCostEnvelope, FrozenSettings};
use systemprompt_identifiers::{EvalExecutionId, ModelId, SessionId};
use systemprompt_scheduler::services::evaluator::adapters::claude_code::ADAPTER;
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

#[test]
fn execution_uses_explicit_files_settings_and_tools_without_ambient_discovery() {
    let args = args(ClientPurpose::Execution, "--dangerously-skip-permissions");
    assert_eq!(args[0], ADAPTER.executable());
    for flag in [
        "--bare",
        "--restricted",
        "--strict-mcp-config",
        "--no-session-persistence",
        "--no-chrome",
    ] {
        assert!(
            args.iter().any(|argument| argument == flag),
            "missing {flag}"
        );
    }
    assert_eq!(value(&args, "--setting-sources"), "");
    assert_eq!(value(&args, "--permission-mode"), "dontAsk");
    assert_eq!(value(&args, "--permission-prompts"), "none");
    assert_eq!(
        value(&args, "--mcp-config"),
        "/home/tester/.claude/evaluator-mcp.json"
    );
    assert_eq!(
        &args[args.len() - 2..],
        &["--", "--dangerously-skip-permissions"]
    );
    assert!(!value(&args, "--tools").contains("Bash"));
    assert!(value(&args, "--allowedTools").contains("mcp__evaluation_fixture__evaluation_fixture"));
    let settings: serde_json::Value = serde_json::from_str(value(&args, "--settings")).unwrap();
    assert_eq!(settings["disableAllHooks"], true);
    assert_eq!(
        settings["permissions"]["disableBypassPermissionsMode"],
        "disable"
    );
    assert_eq!(settings["env"]["CLAUDE_CODE_MAX_OUTPUT_TOKENS"], "4096");
    assert!(
        settings["permissions"]["deny"]
            .as_array()
            .unwrap()
            .iter()
            .any(|rule| rule == "Read(/home/tester/.claude/evaluator-mcp.json)")
    );
}

#[test]
fn judges_and_suggestions_cannot_write_or_invoke_skills_or_mcp_tools() {
    for purpose in [ClientPurpose::Judge, ClientPurpose::Suggestion] {
        let args = args(purpose, "read retained evidence");
        assert_eq!(value(&args, "--tools"), "Read,Glob,Grep");
        assert!(
            args.iter()
                .any(|argument| argument == "--disable-slash-commands")
        );
        assert_eq!(
            value(&args, "--mcp-config"),
            "/home/tester/.claude/evaluator-no-mcp.json"
        );
        for tool in ["Bash", "Write", "Edit", "Skill", "mcp__*"] {
            assert!(
                value(&args, "--disallowedTools")
                    .split(',')
                    .any(|name| name == tool)
            );
        }
    }
    assert_eq!(
        value(&args(ClientPurpose::Judge, "judge"), "--max-turns"),
        "2"
    );
    assert_eq!(
        value(&args(ClientPurpose::Suggestion, "suggest"), "--max-turns"),
        "3"
    );
}

#[test]
fn requested_native_limits_are_enforced_before_argument_construction() {
    let model = ModelId::new("claude-opus-5");
    let limits = ExecutionLimits {
        max_output_tokens: 512,
        max_turns: 1,
        ..ExecutionLimits::default()
    };
    let args = strings(
        ADAPTER
            .arguments(&AdapterInvocation {
                model: &model,
                limits: &limits,
                purpose: ClientPurpose::Judge,
                prompt: "judge",
            })
            .unwrap(),
    );
    assert_eq!(value(&args, "--max-turns"), "1");
    let settings: serde_json::Value = serde_json::from_str(value(&args, "--settings")).unwrap();
    assert_eq!(settings["env"]["CLAUDE_CODE_MAX_OUTPUT_TOKENS"], "512");
    for prompt in ["x".repeat(65_537), "invalid\0prompt".to_owned()] {
        assert!(
            ADAPTER
                .arguments(&AdapterInvocation {
                    model: &model,
                    limits: &limits,
                    purpose: ClientPurpose::Execution,
                    prompt: &prompt,
                })
                .is_err()
        );
    }
}

fn target() -> VerifiedNativeTarget {
    VerifiedNativeTarget {
        client: ClientKind::ClaudeCode,
        platform: "linux".to_owned(),
        architecture: "x86_64".to_owned(),
        client_version: "2.1.270".to_owned(),
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
fn isolated_configuration_uses_execution_credentials_and_native_skill_path() {
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
    let first = ADAPTER
        .configuration(&context)
        .expect("isolated configuration");
    let repeated = ADAPTER
        .configuration(&context)
        .expect("repeat configuration");
    assert_eq!(first.digest().unwrap(), repeated.digest().unwrap());
    assert_eq!(ADAPTER.skill_directory(), ".claude/skills");
    assert_eq!(first.files.len(), 3);
    assert!(first.files.values().all(|file| !file.executable));
    let env = std::str::from_utf8(&first.files["client.env"].bytes).unwrap();
    assert!(env.contains("ANTHROPIC_API_KEY=spexec_fixture.signature\n"));
    assert!(env.contains("ANTHROPIC_AUTH_TOKEN=\n"));
    assert!(env.contains("CLAUDE_CONFIG_DIR=/home/tester/.claude\n"));
    assert!(env.contains("DISABLE_AUTOUPDATER=1\n"));
    let mcp: serde_json::Value =
        serde_json::from_slice(&first.files["home/.claude/evaluator-mcp.json"].bytes).unwrap();
    assert_eq!(
        mcp["mcpServers"]["evaluation_fixture"]["headers"]["x-session-id"],
        session.as_str()
    );
    assert_eq!(
        mcp["mcpServers"]["evaluation_fixture"]["url"],
        "http://eval-relay-fixture:8090/mcp/evaluation_fixture"
    );
    assert!(!format!("{context:?}").contains("spexec_fixture.signature"));
    context.relay_url = "https://api.anthropic.com";
    assert!(ADAPTER.configuration(&context).is_err());
    context.relay_url = "http://eval-relay-fixture:8090";
    context.execution_token = "spexec_fixture\nOPENAI_API_KEY=injected";
    assert!(ADAPTER.configuration(&context).is_err());
}

#[test]
fn version_parsing_requires_exact_claude_release_identity() {
    assert_eq!(
        ADAPTER.parse_version(b"2.1.270 (Claude Code)\n").unwrap(),
        "2.1.270"
    );
    for value in [
        "2.1.270",
        "2.1.270 (Other)",
        "2.1.270-dev (Claude Code)",
        "02.1.270 (Claude Code)",
        "2.1 (Claude Code)",
        "2.1.270 (Claude Code)\n2.1.271 (Claude Code)",
    ] {
        assert!(
            ADAPTER.parse_version(value.as_bytes()).is_err(),
            "accepted {value}"
        );
    }
}

#[test]
fn stream_results_keep_gateway_metering_separate_and_deduplicate_tool_evidence() {
    let assistant = r#"{"type":"assistant","message":{"id":"msg_1","content":[{"type":"tool_use","id":"tool_1","name":"Read","input":{"file_path":"case.json"}}],"usage":{"input_tokens":999}}}"#;
    let result = r#"{"type":"result","subtype":"success","is_error":false,"result":"final answer","usage":{"input_tokens":10,"output_tokens":4}}"#;
    let output = ADAPTER
        .normalize(format!("{assistant}\n{assistant}\n{result}\n{result}\n").as_bytes())
        .unwrap();
    assert_eq!(output.completion, NativeCompletion::Completed);
    assert_eq!(output.text, "final answer");
    assert_eq!(output.tool_calls, vec!["Read"]);
    assert_eq!(output.reported_input_tokens, Some(10));
    assert_eq!(output.reported_output_tokens, Some(4));
}

#[test]
fn failed_incomplete_and_malformed_outputs_cannot_become_completed_judgments() {
    let failure = ADAPTER.normalize(br#"{"type":"result","subtype":"error_max_turns","is_error":true,"errors":["limit reached"],"usage":{"input_tokens":8}}"#).unwrap();
    assert_eq!(failure.completion, NativeCompletion::Failed);
    assert_eq!(failure.reported_input_tokens, Some(8));
    assert_eq!(failure.reported_output_tokens, None);
    let partial = ADAPTER.normalize(br#"{"type":"assistant","message":{"id":"m1","content":[{"type":"text","text":"unfinished"}]}}"#).unwrap();
    assert_eq!(partial.completion, NativeCompletion::Incomplete);
    assert_eq!(partial.text, "unfinished");
    let retained = normalize_evidence(&ADAPTER, b"malformed stream");
    assert_eq!(retained.output.completion, NativeCompletion::Incomplete);
    assert!(retained.diagnostic.is_some());
    assert_eq!(retained.output.reported_input_tokens, None);
}

#[test]
fn malformed_usage_and_conflicting_terminal_evidence_are_rejected() {
    assert!(ADAPTER.normalize(br#"{"type":"result","subtype":"success","result":"text","usage":{"input_tokens":-1}}"#).is_err());
    assert!(ADAPTER.normalize(b"{\"type\":\"result\",\"subtype\":\"success\",\"result\":\"one\"}\n{\"type\":\"result\",\"subtype\":\"success\",\"result\":\"two\"}").is_err());
    assert!(
        ADAPTER
            .normalize(&vec![b'x'; 16 * 1024 * 1024 + 1])
            .is_err()
    );
}
