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

#[test]
fn native_hook_files_contain_host_but_never_device_credentials() {
    let dir = tempfile::tempdir().unwrap();
    let skill = dir.path().join("skills/skill");
    std::fs::create_dir_all(&skill).unwrap();
    std::fs::create_dir_all(dir.path().join("hooks")).unwrap();
    let path = dir.path().join("hooks/hooks.json");
    std::fs::write(&path,r#"{"hooks":{"UserPromptSubmit":[{"hooks":[{"type":"http","headers":{"Authorization":"Bearer loopback","x-systemprompt-device-credential":"must-remove"}}]}]}}"#).unwrap();
    systemprompt_bridge::feedback::hooks::stamp_native_hooks(&skill, EvaluatorClient::ClaudeCode)
        .unwrap();
    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("claude-code"));
    assert!(!content.contains("device-credential"));
    assert!(!content.contains("must-remove"));
}

#[test]
fn forwarded_hook_uses_protected_device_credential_and_strips_caller_credential() {
    let dir = tempfile::tempdir().unwrap();
    temp_env::with_var("XDG_STATE_HOME", Some(dir.path()), || {
        let enrollment = Enrollment::new(
            "https://example.invalid".to_owned(),
            DeviceId::try_new("device").expect("nonempty fixture device"),
            UserId::new("consumer"),
            systemprompt_bridge::ids::BearerToken::new("sp_device_private"),
        )
        .unwrap();
        enrollment
            .save(&systemprompt_bridge::feedback::metadata_root().unwrap())
            .unwrap();
        let mut headers = http::HeaderMap::new();
        headers.insert(
            "x-systemprompt-device-credential",
            http::HeaderValue::from_static("forged"),
        );
        systemprompt_bridge::feedback::hooks::authenticate_forwarded_hook(
            "https://example.invalid",
            Some("claude-code"),
            &mut headers,
        )
        .unwrap();
        assert_eq!(
            headers["x-systemprompt-device-credential"],
            "sp_device_private"
        );
        assert!(headers["x-systemprompt-device-credential"].is_sensitive());
        assert_eq!(headers["x-systemprompt-host"], "claude-code");
        systemprompt_bridge::feedback::hooks::authenticate_forwarded_hook(
            "https://example.invalid",
            None,
            &mut headers,
        )
        .unwrap();
        assert!(!headers.contains_key("x-systemprompt-device-credential"));
    });
}
