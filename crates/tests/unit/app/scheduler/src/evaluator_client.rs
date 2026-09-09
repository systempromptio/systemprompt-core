//! Argument construction and limit validation for the pinned native clients.
//!
//! The evaluator never lets suite data supply a shell command: the argument
//! vector is built from a fixed template plus the pinned model, so these tests
//! assert the shape of that vector per client kind and that the builder
//! refuses limits outside the supported envelope.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::ffi::OsString;
use systemprompt_evaluation::experiments::ClientKind;
use systemprompt_evaluation::experiments::execution::ExecutionLimits;
use systemprompt_identifiers::ModelId;
use systemprompt_scheduler::services::evaluator::client::NativeClient;

fn strings(arguments: &[OsString]) -> Vec<String> {
    arguments
        .iter()
        .map(|value| value.to_string_lossy().into_owned())
        .collect()
}

fn claude_client() -> NativeClient {
    NativeClient::builder(ClientKind::ClaudeCode, ModelId::new("claude-opus-5"))
        .build()
        .expect("default execution limits are within the supported envelope")
}

#[test]
fn claude_code_arguments_pin_the_model_and_end_with_the_prompt() {
    let arguments = strings(&claude_client().arguments("audit the repository"));

    assert_eq!(
        arguments.first().map(String::as_str),
        Some("claude"),
        "the executable is the fixed client name, never suite-supplied"
    );
    assert_eq!(
        arguments.last().map(String::as_str),
        Some("audit the repository"),
        "the prompt must be the final argument, after the `--` terminator"
    );
    let terminator = arguments
        .iter()
        .position(|value| value == "--")
        .expect("a `--` terminator must separate flags from the prompt");
    assert_eq!(
        terminator,
        arguments.len() - 2,
        "nothing may sit between the terminator and the prompt"
    );
    let model = arguments
        .iter()
        .position(|value| value == "--model")
        .expect("the model must be passed explicitly");
    assert_eq!(arguments[model + 1], "claude-opus-5");
}

#[test]
fn claude_code_arguments_deny_the_dangerous_tools() {
    let arguments = strings(&claude_client().arguments("prompt"));
    let disallowed = arguments
        .iter()
        .position(|value| value == "--disallowedTools")
        .expect("the dangerous tools must be denied explicitly");

    for tool in ["Bash", "Agent", "Task", "WebSearch", "WebFetch"] {
        assert!(
            arguments[disallowed + 1]
                .split(',')
                .any(|name| name == tool),
            "{tool} must stay denied, got {}",
            arguments[disallowed + 1]
        );
    }
    let allowed = arguments
        .iter()
        .position(|value| value == "--allowedTools")
        .expect("the permitted tools must be listed explicitly");
    assert!(
        !arguments[allowed + 1].split(',').any(|name| name == "Bash"),
        "Bash must never appear in the allow list"
    );
}

#[test]
fn claude_code_arguments_carry_the_configured_turn_limit() {
    let client = NativeClient::builder(ClientKind::ClaudeCode, ModelId::new("claude-opus-5"))
        .limits(ExecutionLimits {
            max_turns: 7,
            ..ExecutionLimits::default()
        })
        .build()
        .expect("seven turns is inside the supported envelope");
    let arguments = strings(&client.arguments("prompt"));
    let turns = arguments
        .iter()
        .position(|value| value == "--max-turns")
        .expect("the turn limit must be passed to the client");

    assert_eq!(arguments[turns + 1], "7");
    assert_eq!(client.limits().max_turns, 7);
}

#[test]
fn opencode_arguments_namespace_the_model_to_the_gateway() {
    let client = NativeClient::builder(ClientKind::Opencode, ModelId::new("gpt-5"))
        .build()
        .expect("default execution limits are within the supported envelope");
    let arguments = strings(&client.arguments("prompt"));

    assert_eq!(arguments.first().map(String::as_str), Some("opencode"));
    let model = arguments
        .iter()
        .position(|value| value == "--model")
        .expect("the model must be passed explicitly");
    assert_eq!(
        arguments[model + 1],
        "systemprompt/gpt-5",
        "opencode must be routed through the systemprompt gateway provider"
    );
    assert_eq!(arguments.last().map(String::as_str), Some("prompt"));
}

#[test]
fn the_builder_refuses_limits_outside_the_envelope() {
    for limits in [
        ExecutionLimits {
            max_turns: 0,
            ..ExecutionLimits::default()
        },
        ExecutionLimits {
            active_timeout_seconds: 0,
            ..ExecutionLimits::default()
        },
        ExecutionLimits {
            max_artifact_bytes: 64 * 1024 * 1024,
            ..ExecutionLimits::default()
        },
    ] {
        assert!(
            NativeClient::builder(ClientKind::ClaudeCode, ModelId::new("claude-opus-5"))
                .limits(limits)
                .build()
                .is_err(),
            "a client must not be constructible with limits outside the envelope"
        );
    }
}
