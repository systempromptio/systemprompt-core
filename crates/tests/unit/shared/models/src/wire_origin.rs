//! Request-origin attribution: the evidence ladder the gateway runs, and the
//! closed enums behind `ai_requests.client_kind` /
//! `ai_requests.wire_protocol` / `ai_requests.client_attestation`.
//!
//! These tests pin the ladder's precedence, its rejections, that the
//! User-Agent tier matches an exact product token and never a substring,
//! that every variant round-trips through its column string, and that the
//! bridge host ids map onto the wire vocabulary.

use systemprompt_models::bridge::profile::KNOWN_HOSTS;
use systemprompt_models::feedback::EvaluatorClient;
use systemprompt_models::wire::origin::{
    ClassificationInput, ClassificationRejection, ClientAttestation, ClientKind,
    InboundWireProtocol, NativeMarker, OriginParseError, RequestOrigin, StainlessHeaders, classify,
    native_marker, ua_product,
};

const CLAUDE_BODY: &[u8] = br#"{"metadata":{"user_id":"user_ab12_account_3f2504e0-4f89-11d3-9a0c-0305e82c3301_session_6ba7b810-9dad-11d1-80b4-00c04fd430c8"}}"#;
const CODEX_BODY: &[u8] =
    br#"{"client_metadata":{"x-codex-turn-metadata":"{\"thread_id\":\"t\"}"}}"#;
/// Claude Code ≥ 2.1.25x: the session rides as a JSON string.
const CLAUDE_JSON_BODY: &[u8] = br#"{"metadata":{"user_id":"{\"account_uuid\":\"\",\"device_id\":\"3c2a1f\",\"session_id\":\"6ba7b810-9dad-11d1-80b4-00c04fd430c8\"}"}}"#;
/// A Cowork (Claude Desktop local agent) turn on a third-party gateway.
const COWORK_BODY: &[u8] = br#"{"system":[{"type":"text","text":"x-anthropic-billing-header: cc_version=2.1.260.07b; cc_entrypoint=local-agent;\nYou are Claude Code"}],"metadata":{"user_id":"{\"account_uuid\":\"\",\"device_id\":\"3c2a1f\",\"session_id\":\"6ba7b810-9dad-11d1-80b4-00c04fd430c8\"}"}}"#;
const DESKTOP_BODY: &[u8] = br#"{"system":"x-anthropic-billing-header: cc_version=2.1.270.0e5; cc_entrypoint=claude-desktop-3p;","messages":[]}"#;
const CLI_BODY: &[u8] = br#"{"system":[{"type":"text","text":"x-anthropic-billing-header: cc_version=2.1.274.1a2; cc_entrypoint=cli;"}],"messages":[]}"#;

fn input<'a>(body: &'a [u8]) -> ClassificationInput<'a> {
    ClassificationInput {
        principal_is_bridge: false,
        declared_client: None,
        declared_attestation: None,
        user_agent: None,
        stainless: StainlessHeaders::default(),
        body,
    }
}

#[test]
fn user_agent_matches_the_exact_first_product_token_only() {
    let cases = [
        (
            "claude-cli/2.0.1 (external, cli)",
            Some(ClientKind::ClaudeCode),
        ),
        ("Claude-Code/1.0", Some(ClientKind::ClaudeCode)),
        ("claude-desktop/0.9", Some(ClientKind::ClaudeDesktop)),
        ("opencode/0.4.2", Some(ClientKind::OpenCode)),
        ("codex_cli_rs/0.2", Some(ClientKind::Codex)),
        ("Hermes-Agent/1.0", Some(ClientKind::Hermes)),
        ("anthropic-sdk-python/0.40", None),
        ("curl/8.5", None),
        ("Mozilla/5.0 (compatible; claude-cli/2.0)", None),
        ("my-opencode-fork/1.0", None),
        ("claude-cli-wrapper/1.0", None),
    ];
    for (agent, expected) in cases {
        let mut i = input(b"{}");
        i.user_agent = Some(agent);
        let classified = classify(&i).expect(agent);
        let (client, attestation) = expected
            .map_or((ClientKind::Other, ClientAttestation::None), |kind| {
                (kind, ClientAttestation::UserAgent)
            });
        assert_eq!(classified.client, client, "{agent}");
        assert_eq!(classified.attestation, attestation, "{agent}");
        assert_eq!(classified.evidence.kind_source, attestation, "{agent}");
    }
    assert_eq!(
        ua_product(Some("claude-cli/2.0.1 (external, cli)")),
        Some(("claude-cli".to_owned(), Some("2.0.1".to_owned())))
    );
    assert_eq!(ua_product(Some("curl")), Some(("curl".to_owned(), None)));
    assert_eq!(ua_product(Some("")), None);
    assert_eq!(
        ua_product(Some("bad token/1")),
        Some(("bad".to_owned(), None))
    );
    assert_eq!(ua_product(Some("\u{1F600}/1")), None);
}

#[test]
fn native_markers_are_structural() {
    assert_eq!(
        native_marker(CLAUDE_BODY),
        Some(NativeMarker::ClaudeMetadataUserId)
    );
    assert_eq!(
        native_marker(CODEX_BODY),
        Some(NativeMarker::CodexTurnMetadata)
    );
    assert_eq!(
        native_marker(CLAUDE_JSON_BODY),
        Some(NativeMarker::ClaudeMetadataJson)
    );
    assert_eq!(
        native_marker(COWORK_BODY),
        Some(NativeMarker::ClaudeDesktopEntrypoint)
    );
    assert_eq!(
        native_marker(DESKTOP_BODY),
        Some(NativeMarker::ClaudeDesktopEntrypoint)
    );
    assert_eq!(
        native_marker(CLI_BODY),
        Some(NativeMarker::ClaudeCliEntrypoint)
    );
    assert_eq!(
        native_marker(
            br#"{"system":"x-anthropic-billing-header: cc_version=1; cc_entrypoint=sdk-py;"}"#
        ),
        None
    );
    assert_eq!(
        native_marker(br#"{"system":"Hi. x-anthropic-billing-header: cc_entrypoint=cli;"}"#),
        None
    );
    assert_eq!(
        native_marker(br#"{"metadata":{"user_id":"{\"session_id\":\"6ba7b810-9dad-11d1-80b4-00c04fd430c8\"}"}}"#),
        None
    );
    assert_eq!(
        native_marker(br#"{"metadata":{"user_id":"user_x_account_notauuid_session_y"}}"#),
        None
    );
    assert_eq!(native_marker(br#"{"metadata":{"user_id":"{}"}}"#), None);
    assert_eq!(native_marker(b""), None);
    assert_eq!(native_marker(b"not json"), None);
    assert_eq!(native_marker(b"[1,2]"), None);
    for marker in NativeMarker::ALL {
        assert_eq!(NativeMarker::parse(marker.as_str()), Ok(marker));
    }
}

#[test]
fn native_marker_beats_user_agent_and_records_the_tier() {
    let mut i = input(CODEX_BODY);
    i.user_agent = Some("Mozilla/5.0");
    let classified = classify(&i).expect("classified");
    assert_eq!(classified.client, ClientKind::Codex);
    assert_eq!(classified.attestation, ClientAttestation::NativeMarker);
    assert!(!classified.conflicting);

    let mut i = input(CLAUDE_BODY);
    i.user_agent = Some("claude-cli/2.0");
    let classified = classify(&i).expect("classified");
    assert_eq!(classified.client, ClientKind::ClaudeCode);
    assert_eq!(classified.attestation, ClientAttestation::NativeMarker);
    assert_eq!(
        classified.evidence.ua_product.as_deref(),
        Some("claude-cli")
    );

    // A Cowork turn wears the CLI User-Agent; the entrypoint names the host.
    let mut i = input(COWORK_BODY);
    i.user_agent = Some("claude-cli/2.1.260");
    let classified = classify(&i).expect("classified");
    assert_eq!(classified.client, ClientKind::ClaudeDesktop);
    assert_eq!(classified.attestation, ClientAttestation::NativeMarker);
    assert_eq!(
        classified.evidence.native_marker,
        Some(NativeMarker::ClaudeDesktopEntrypoint)
    );
    assert!(!classified.conflicting);

    let classified = classify(&input(CLAUDE_JSON_BODY)).expect("classified");
    assert_eq!(classified.client, ClientKind::ClaudeCode);
    assert_eq!(classified.attestation, ClientAttestation::NativeMarker);
}

#[test]
fn declaration_beats_marker_and_user_agent_and_flags_the_conflict() {
    let mut i = input(CODEX_BODY);
    i.user_agent = Some("claude-cli/2.0");
    i.declared_client = Some("pi");
    let classified = classify(&i).expect("classified");
    assert_eq!(classified.client, ClientKind::Pi);
    assert_eq!(classified.attestation, ClientAttestation::Declared);
    assert_eq!(classified.evidence.kind_source, ClientAttestation::Declared);
    assert_eq!(classified.evidence.declared_client.as_deref(), Some("pi"));
    assert_eq!(
        classified.evidence.native_marker,
        Some(NativeMarker::CodexTurnMetadata)
    );
    assert!(classified.conflicting);
}

#[test]
fn malformed_declaration_is_rejected_and_kept_as_evidence() {
    let mut i = input(b"{}");
    i.declared_client = Some("Pi");
    let rejection = classify(&i).expect_err("rejected");
    assert!(matches!(
        rejection,
        ClassificationRejection::MalformedDeclaredClient { .. }
    ));
    let message = rejection.to_string();
    for kind in ClientKind::DECLARABLE {
        assert!(message.contains(kind.as_str()), "{message}");
    }
    assert!(!message.contains("internal"), "{message}");
    assert_eq!(rejection.evidence().declared_client.as_deref(), Some("Pi"));

    i.declared_client = Some("internal");
    assert!(classify(&i).is_err());
    i.declared_client = Some("unknown");
    assert!(classify(&i).is_err());
    i.declared_client = Some("other");
    assert_eq!(classify(&i).expect("other").client, ClientKind::Other);
}

#[test]
fn attestation_header_is_bridge_only() {
    let mut i = input(b"{}");
    i.declared_attestation = Some("host-token");
    i.declared_client = Some("opencode");
    let rejection = classify(&i).expect_err("rejected");
    assert!(matches!(
        rejection,
        ClassificationRejection::AttestationNotFromBridge { .. }
    ));
    assert!(rejection.to_string().contains("bridge only"));

    i.principal_is_bridge = true;
    i.declared_attestation = Some("declared");
    assert!(matches!(
        classify(&i),
        Err(ClassificationRejection::MalformedAttestation { .. })
    ));
    i.declared_attestation = Some("host-token");
    i.declared_client = None;
    assert!(matches!(
        classify(&i),
        Err(ClassificationRejection::HostTokenWithoutClient { .. })
    ));
}

#[test]
fn host_token_wins_over_everything_and_names_the_attested_host() {
    let mut i = input(CLAUDE_BODY);
    i.principal_is_bridge = true;
    i.user_agent = Some("claude-cli/2.0");
    i.declared_attestation = Some("host-token");
    i.declared_client = Some("opencode");
    let classified = classify(&i).expect("classified");
    assert_eq!(classified.client, ClientKind::OpenCode);
    assert_eq!(classified.attestation, ClientAttestation::HostToken);
    assert_eq!(
        classified.evidence.kind_source,
        ClientAttestation::HostToken
    );
    assert_eq!(
        classified.evidence.attested_host,
        Some(ClientKind::OpenCode)
    );
    assert_eq!(
        classified.evidence.native_marker,
        Some(NativeMarker::ClaudeMetadataUserId)
    );
    assert!(classified.conflicting);
}

#[test]
fn bridge_secret_is_a_channel_fact_beside_the_naming_tier() {
    let mut i = input(CLAUDE_BODY);
    i.principal_is_bridge = true;
    i.declared_attestation = Some("bridge-secret");
    let classified = classify(&i).expect("classified");
    assert_eq!(classified.client, ClientKind::ClaudeCode);
    assert_eq!(classified.attestation, ClientAttestation::BridgeSecret);
    assert_eq!(
        classified.evidence.kind_source,
        ClientAttestation::NativeMarker
    );
    assert_eq!(classified.evidence.attested_host, None);

    i.declared_client = Some("pi");
    let classified = classify(&i).expect("classified");
    assert_eq!(classified.client, ClientKind::Pi);
    assert_eq!(classified.attestation, ClientAttestation::BridgeSecret);
    assert_eq!(classified.evidence.kind_source, ClientAttestation::Declared);

    let mut bare = input(b"{}");
    bare.principal_is_bridge = true;
    bare.declared_attestation = Some("bridge-secret");
    let classified = classify(&bare).expect("classified");
    assert_eq!(classified.client, ClientKind::Other);
    assert_eq!(classified.attestation, ClientAttestation::BridgeSecret);
    assert_eq!(classified.evidence.kind_source, ClientAttestation::None);
}

#[test]
fn evidence_strings_are_bounded_and_sdk_headers_are_kept() {
    let long = "x".repeat(4096);
    let mut i = input(b"{}");
    i.user_agent = Some(long.as_str());
    i.stainless = StainlessHeaders {
        lang: Some("python"),
        package_version: Some(long.as_str()),
        runtime: Some(" CPython "),
        runtime_version: Some(""),
        os: Some("Linux"),
        arch: Some("x64"),
    };
    let classified = classify(&i).expect("classified");
    assert_eq!(classified.client, ClientKind::Other);
    assert_eq!(
        classified.evidence.ua_product.as_deref().map(str::len),
        Some(64)
    );
    assert_eq!(
        classified
            .evidence
            .sdk_package_version
            .as_deref()
            .map(str::len),
        Some(64)
    );
    assert_eq!(classified.evidence.sdk_runtime.as_deref(), Some("CPython"));
    assert_eq!(classified.evidence.sdk_runtime_version, None);
    assert_eq!(classified.evidence.sdk_lang.as_deref(), Some("python"));
}

#[test]
fn classifier_is_total_and_never_server_only() {
    let long = "x".repeat(4096);
    let agents = [None, Some(""), Some("\u{1F600}"), Some(long.as_str())];
    let bodies: [&[u8]; 4] = [b"", b"not json", b"[1,2]", b"{\"client_metadata\":{}}"];
    for agent in agents {
        for body in bodies {
            let mut i = input(body);
            i.user_agent = agent;
            let classified = classify(&i).expect("no headers never rejects");
            assert!(
                !matches!(
                    classified.client,
                    ClientKind::Internal | ClientKind::Unknown
                ) && !matches!(
                    classified.attestation,
                    ClientAttestation::Internal | ClientAttestation::Unknown
                ),
                "{agent:?} / {body:?} produced {classified:?}"
            );
        }
    }
}

#[test]
fn every_enum_round_trips_through_its_column_string() {
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

    for wire in InboundWireProtocol::ALL {
        assert_eq!(InboundWireProtocol::parse(wire.as_str()), Ok(wire));
    }
    assert_eq!(
        InboundWireProtocol::parse("grpc"),
        Err(OriginParseError::WireProtocol("grpc".to_owned()))
    );

    for tier in ClientAttestation::ALL {
        assert_eq!(ClientAttestation::parse(tier.as_str()), Ok(tier));
    }
    assert_eq!(
        ClientAttestation::parse("Host-Token"),
        Err(OriginParseError::Attestation("Host-Token".to_owned()))
    );
    assert!(ClientAttestation::HostToken < ClientAttestation::Declared);
    assert!(ClientAttestation::Declared < ClientAttestation::UserAgent);
}

#[test]
fn serde_uses_the_column_strings() {
    let origin = RequestOrigin::gateway(
        ClientKind::OpenCode,
        InboundWireProtocol::OpenAiChat,
        ClientAttestation::HostToken,
    );
    let json = serde_json::to_value(origin).expect("serialize");
    assert_eq!(
        json,
        serde_json::json!({"client": "opencode", "wire": "openai.chat", "attestation": "host-token"})
    );
    let back: RequestOrigin = serde_json::from_value(json).expect("deserialize");
    assert_eq!(back, origin);
}

#[test]
fn internal_origin_pairs_all_three_halves() {
    assert_eq!(RequestOrigin::INTERNAL.client, ClientKind::Internal);
    assert_eq!(RequestOrigin::INTERNAL.wire, InboundWireProtocol::Internal);
    assert_eq!(
        RequestOrigin::INTERNAL.attestation,
        ClientAttestation::Internal
    );
}

#[test]
fn every_known_bridge_host_maps_onto_the_wire_vocabulary() {
    for host in KNOWN_HOSTS {
        let kind = ClientKind::from_bridge_host_id(host)
            .unwrap_or_else(|| panic!("{host} has no ClientKind"));
        assert_eq!(kind.bridge_host_id(), Some(*host));
        assert!(EvaluatorClient::try_from(kind).is_ok(), "{host}");
    }
    assert_eq!(ClientKind::from_bridge_host_id("codex"), None);
    assert_eq!(ClientKind::Pi.bridge_host_id(), None);
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
    for kind in [
        ClientKind::Pi,
        ClientKind::Other,
        ClientKind::Internal,
        ClientKind::Unknown,
    ] {
        assert_eq!(
            EvaluatorClient::try_from(kind),
            Err(OriginParseError::NotAHarness(kind.as_str()))
        );
    }
}
