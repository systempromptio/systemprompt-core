use super::*;

#[test]
fn native_session_binding_does_not_invent_proxy_sessions_and_detects_conflicting_aliases() {
    let mut headers = http::HeaderMap::new();
    headers.insert("x-session-id", "synthesized-proxy-session".parse().unwrap());
    assert!(native_session(&headers, b"{}").is_none());
    headers.insert("user-agent", "codex_cli_rs/1.0".parse().unwrap());
    assert!(native_session(&headers, b"{}").is_none());
    headers.insert("session-id", "native-session".parse().unwrap());
    assert_eq!(
        native_session(&headers, b"{}").unwrap().id.as_str(),
        "native-session"
    );
    headers.insert("session_id", "conflicting-native-session".parse().unwrap());
    assert!(native_session(&headers, b"{}").is_none());
    let codex=serde_json::to_vec(&serde_json::json!({"client_metadata":{"x-codex-turn-metadata":"{\"thread_id\":\"thread-123\"}"}})).unwrap();
    assert_eq!(
        native_session(&http::HeaderMap::new(), &codex)
            .unwrap()
            .id
            .as_str(),
        "thread-123"
    );
    let mut opencode = http::HeaderMap::new();
    opencode.insert("user-agent", "opencode/1.0".parse().unwrap());
    opencode.insert("x-opencode-session", "ses_123".parse().unwrap());
    assert_eq!(
        native_session(&opencode, b"{}").unwrap().host,
        EvaluatorClient::OpenCode
    );
    let mut hermes = http::HeaderMap::new();
    hermes.insert("user-agent", "HermesAgent/1.0".parse().unwrap());
    assert_eq!(
        native_session(&hermes, br#"{"session_id":"hermes-session"}"#)
            .unwrap()
            .host,
        EvaluatorClient::Hermes
    );
}

#[test]
fn session_bindings_remain_frozen_to_generation_observed_at_native_session_start() {
    let (dir, receipt) = prepared(EvaluatorClient::Codex);
    let outbox = Outbox::new(dir.path().join("outbox.json"), scope("device"));
    outbox.enqueue(receipt.clone()).unwrap();
    outbox
        .queue_session(EvaluatorClient::Codex, "session")
        .unwrap();
    let mut upgrade = receipt;
    upgrade.generation = 2;
    upgrade.publication_id = PublicationId::new("upgraded");
    outbox.enqueue(upgrade).unwrap();
    outbox
        .queue_session(EvaluatorClient::Codex, "session")
        .unwrap();
    for (_, entry) in outbox.entries().unwrap() {
        assert_eq!(
            entry.session_bindings.contains_key("session"),
            entry.request.generation == 1
        );
    }
}

#[test]
fn bounded_outbox_never_discards_pending_receipts() {
    let (dir, receipt) = prepared(EvaluatorClient::Codex);
    let outbox = Outbox::new(dir.path().join("outbox.json"), scope("device"));
    for generation in 1..=512 {
        let mut next = receipt.clone();
        next.generation = generation;
        next.publication_id = PublicationId::new(format!("p-{generation}"));
        outbox.enqueue(next).unwrap();
    }
    let mut next = receipt;
    next.publication_id = PublicationId::new("overflow");
    assert!(matches!(outbox.enqueue(next), Err(FeedbackError::Full)));
    assert_eq!(outbox.entries().unwrap().len(), 512);
}

#[test]
fn pending_native_session_binds_only_publication_present_when_observed() {
    let (dir, receipt) = prepared(EvaluatorClient::Codex);
    let outbox = Outbox::new(dir.path().join("outbox.json"), scope("device"));
    outbox
        .reserve_installation(
            systemprompt_bridge::feedback::outbox::PendingInstallation::new(
                publication(),
                EvaluatorClient::Codex,
                vec![dir.path().to_path_buf()],
            ),
        )
        .unwrap();
    outbox
        .queue_session(EvaluatorClient::Codex, "offline-session")
        .unwrap();
    outbox.enqueue(receipt.clone()).unwrap();
    let mut upgrade = receipt;
    upgrade.generation = 2;
    upgrade.publication_id = PublicationId::new("upgraded");
    outbox.enqueue(upgrade).unwrap();
    for (_, entry) in outbox.entries().unwrap() {
        assert_eq!(
            entry.session_bindings.contains_key("offline-session"),
            entry.request.generation == 1
        );
    }
}

#[test]
fn session_without_known_installation_does_not_acquire_future_publication() {
    let (dir, receipt) = prepared(EvaluatorClient::Codex);
    let outbox = Outbox::new(dir.path().join("outbox.json"), scope("device"));
    outbox
        .queue_session(EvaluatorClient::Codex, "unknown-session")
        .unwrap();
    outbox.enqueue(receipt).unwrap();
    assert!(outbox.entries().unwrap()[0].1.session_bindings.is_empty());
}
