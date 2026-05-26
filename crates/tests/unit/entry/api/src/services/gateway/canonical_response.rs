//! Unit tests for `CanonicalStopReason` — round-trip between Anthropic and
//! OpenAI wire codes plus the `Role::as_str` mapping.

use systemprompt_api::services::gateway::protocol::canonical::Role;
use systemprompt_api::services::gateway::protocol::canonical_response::{
    CanonicalStopReason, CanonicalUsage,
};

#[test]
fn role_as_str() {
    assert_eq!(Role::System.as_str(), "system");
    assert_eq!(Role::User.as_str(), "user");
    assert_eq!(Role::Assistant.as_str(), "assistant");
    assert_eq!(Role::Tool.as_str(), "tool");
}

#[test]
fn anthropic_str_mapping() {
    assert_eq!(CanonicalStopReason::EndTurn.anthropic_str(), "end_turn");
    assert_eq!(CanonicalStopReason::MaxTokens.anthropic_str(), "max_tokens");
    assert_eq!(
        CanonicalStopReason::StopSequence.anthropic_str(),
        "stop_sequence"
    );
    assert_eq!(CanonicalStopReason::ToolUse.anthropic_str(), "tool_use");
    assert_eq!(CanonicalStopReason::Other.anthropic_str(), "end_turn");
}

#[test]
fn openai_str_mapping() {
    assert_eq!(CanonicalStopReason::EndTurn.openai_str(), "stop");
    assert_eq!(CanonicalStopReason::MaxTokens.openai_str(), "length");
    assert_eq!(CanonicalStopReason::StopSequence.openai_str(), "stop");
    assert_eq!(CanonicalStopReason::ToolUse.openai_str(), "tool_calls");
    assert_eq!(CanonicalStopReason::Other.openai_str(), "stop");
}

#[test]
fn from_anthropic_known_codes() {
    assert_eq!(
        CanonicalStopReason::from_anthropic("end_turn"),
        CanonicalStopReason::EndTurn
    );
    assert_eq!(
        CanonicalStopReason::from_anthropic("max_tokens"),
        CanonicalStopReason::MaxTokens
    );
    assert_eq!(
        CanonicalStopReason::from_anthropic("stop_sequence"),
        CanonicalStopReason::StopSequence
    );
    assert_eq!(
        CanonicalStopReason::from_anthropic("tool_use"),
        CanonicalStopReason::ToolUse
    );
}

#[test]
fn from_anthropic_unknown_is_other() {
    assert_eq!(
        CanonicalStopReason::from_anthropic("garbage"),
        CanonicalStopReason::Other
    );
    assert_eq!(
        CanonicalStopReason::from_anthropic(""),
        CanonicalStopReason::Other
    );
}

#[test]
fn from_openai_known_codes() {
    assert_eq!(
        CanonicalStopReason::from_openai("stop"),
        CanonicalStopReason::EndTurn
    );
    assert_eq!(
        CanonicalStopReason::from_openai("length"),
        CanonicalStopReason::MaxTokens
    );
    assert_eq!(
        CanonicalStopReason::from_openai("tool_calls"),
        CanonicalStopReason::ToolUse
    );
    assert_eq!(
        CanonicalStopReason::from_openai("function_call"),
        CanonicalStopReason::ToolUse
    );
}

#[test]
fn from_openai_unknown_is_other() {
    assert_eq!(
        CanonicalStopReason::from_openai("custom"),
        CanonicalStopReason::Other
    );
}

#[test]
fn anthropic_round_trip_for_known_codes() {
    for code in ["end_turn", "max_tokens", "stop_sequence", "tool_use"] {
        let rt = CanonicalStopReason::from_anthropic(code).anthropic_str();
        assert_eq!(rt, code);
    }
}

#[test]
fn canonical_usage_default() {
    let u = CanonicalUsage::default();
    assert_eq!(u.input_tokens, 0);
    assert_eq!(u.output_tokens, 0);
}
