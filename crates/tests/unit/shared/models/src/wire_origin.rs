//! Request-origin attribution: the one classifier the bridge and the gateway
//! share, and the closed enums behind `ai_requests.client_kind` /
//! `ai_requests.wire_protocol`.
//!
//! These tests pin three properties: the classifier is total and never yields
//! a server-only variant, every variant round-trips through its column string,
//! and the harness mapping to `EvaluatorClient` covers exactly the harnesses.

use systemprompt_models::feedback::EvaluatorClient;
use systemprompt_models::wire::origin::{
    ClientKind, InboundWireProtocol, OriginParseError, RequestOrigin,
};

#[test]
fn user_agents_map_to_their_harness() {
    let cases = [
        ("claude-cli/2.0.1 (external, cli)", ClientKind::ClaudeCode),
        ("Claude-Code/1.0", ClientKind::ClaudeCode),
        ("claude-desktop/0.9", ClientKind::ClaudeDesktop),
        ("opencode/0.4.2", ClientKind::OpenCode),
        ("codex_cli_rs/0.2", ClientKind::Codex),
        ("Hermes-Agent/1.0", ClientKind::Hermes),
        ("anthropic-sdk-python/0.40", ClientKind::Other),
        ("curl/8.5", ClientKind::Other),
    ];
    for (agent, expected) in cases {
        assert_eq!(
            ClientKind::from_user_agent_and_body(Some(agent), b"{}"),
            expected,
            "{agent}"
        );
    }
}

#[test]
fn codex_body_marker_is_used_only_when_no_agent_matched() {
    let body = br#"{"client_metadata":{"x-codex-turn-metadata":"{\"thread_id\":\"t\"}"}}"#;
    assert_eq!(
        ClientKind::from_user_agent_and_body(None, body),
        ClientKind::Codex
    );
    assert_eq!(
        ClientKind::from_user_agent_and_body(Some("Mozilla/5.0"), body),
        ClientKind::Codex
    );
    assert_eq!(
        ClientKind::from_user_agent_and_body(Some("claude-cli/2.0"), body),
        ClientKind::ClaudeCode
    );
}

#[test]
fn classifier_is_total_and_never_server_only() {
    let long = "x".repeat(4096);
    let agents = [None, Some(""), Some("\u{1F600}"), Some(long.as_str())];
    let bodies: [&[u8]; 4] = [b"", b"not json", b"[1,2]", b"{\"client_metadata\":{}}"];
    for agent in agents {
        for body in bodies {
            let kind = ClientKind::from_user_agent_and_body(agent, body);
            assert!(
                !matches!(kind, ClientKind::Internal | ClientKind::Unknown),
                "{agent:?} / {body:?} produced {kind:?}"
            );
        }
    }
    assert_eq!(
        ClientKind::from_user_agent_and_body(None, b"not json"),
        ClientKind::Other
    );
}

#[test]
fn client_kind_round_trips_through_its_column_string() {
    for kind in ClientKind::ALL {
        assert_eq!(ClientKind::parse(kind.as_str()), Ok(kind));
    }
    assert_eq!(
        ClientKind::parse("gpt"),
        Err(OriginParseError::ClientKind("gpt".to_owned()))
    );
    let strings: std::collections::BTreeSet<&str> =
        ClientKind::ALL.iter().map(|kind| kind.as_str()).collect();
    assert_eq!(
        strings.len(),
        ClientKind::ALL.len(),
        "column strings collide"
    );
}

#[test]
fn wire_protocol_round_trips_and_keeps_log_names() {
    for wire in InboundWireProtocol::ALL {
        assert_eq!(InboundWireProtocol::parse(wire.as_str()), Ok(wire));
    }
    assert_eq!(
        InboundWireProtocol::AnthropicMessages.as_str(),
        "anthropic.messages"
    );
    assert_eq!(InboundWireProtocol::OpenAiChat.as_str(), "openai.chat");
    assert_eq!(
        InboundWireProtocol::OpenAiResponses.as_str(),
        "openai.responses"
    );
    assert_eq!(
        InboundWireProtocol::parse("grpc"),
        Err(OriginParseError::WireProtocol("grpc".to_owned()))
    );
}

#[test]
fn serde_uses_the_column_strings() {
    let origin = RequestOrigin::gateway(ClientKind::OpenCode, InboundWireProtocol::OpenAiChat);
    let json = serde_json::to_value(origin).expect("serialize");
    assert_eq!(
        json,
        serde_json::json!({"client": "opencode", "wire": "openai.chat"})
    );
    let back: RequestOrigin = serde_json::from_value(json).expect("deserialize");
    assert_eq!(back, origin);
}

#[test]
fn internal_origin_pairs_both_halves() {
    assert_eq!(RequestOrigin::INTERNAL.client, ClientKind::Internal);
    assert_eq!(RequestOrigin::INTERNAL.wire, InboundWireProtocol::Internal);
}

#[test]
fn evaluator_client_maps_exactly_the_harnesses() {
    let harnesses = [
        EvaluatorClient::ClaudeCode,
        EvaluatorClient::ClaudeDesktop,
        EvaluatorClient::Codex,
        EvaluatorClient::OpenCode,
        EvaluatorClient::Hermes,
    ];
    for harness in harnesses {
        let kind = ClientKind::from(harness);
        assert_eq!(EvaluatorClient::try_from(kind), Ok(harness));
    }
    for kind in [ClientKind::Other, ClientKind::Internal, ClientKind::Unknown] {
        assert_eq!(
            EvaluatorClient::try_from(kind),
            Err(OriginParseError::NotAHarness(kind.as_str()))
        );
    }
}
