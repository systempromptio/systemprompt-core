use super::*;
use systemprompt_bridge::ids::HostId;

fn host(id: &str) -> HostId {
    HostId::new(id)
}

#[test]
fn native_session_binding_does_not_invent_proxy_sessions_and_detects_conflicting_aliases() {
    let mut headers = http::HeaderMap::new();
    headers.insert("x-session-id", "synthesized-proxy-session".parse().unwrap());
    assert!(native_session(None, &headers, b"{}").is_none());
    headers.insert("user-agent", "codex_cli_rs/1.0".parse().unwrap());
    assert!(native_session(None, &headers, b"{}").is_none());
    headers.insert("session-id", "native-session".parse().unwrap());
    assert_eq!(
        native_session(None, &headers, b"{}").unwrap().id.as_str(),
        "native-session"
    );
    headers.insert("session_id", "conflicting-native-session".parse().unwrap());
    assert!(native_session(None, &headers, b"{}").is_none());
    let codex=serde_json::to_vec(&serde_json::json!({"client_metadata":{"x-codex-turn-metadata":"{\"thread_id\":\"thread-123\"}"}})).unwrap();
    assert_eq!(
        native_session(None, &http::HeaderMap::new(), &codex)
            .unwrap()
            .id
            .as_str(),
        "thread-123"
    );
    let mut opencode = http::HeaderMap::new();
    opencode.insert("user-agent", "opencode/1.0".parse().unwrap());
    opencode.insert("x-opencode-session", "ses_123".parse().unwrap());
    assert_eq!(
        native_session(None, &opencode, b"{}").unwrap().host,
        EvaluatorClient::OpenCode
    );
    let hermes = http::HeaderMap::new();
    assert_eq!(
        native_session(
            Some(&host("hermes")),
            &hermes,
            br#"{"session_id":"hermes-session"}"#
        )
        .unwrap()
        .host,
        EvaluatorClient::Hermes
    );
}

#[test]
fn a_verified_host_token_names_the_host_over_body_and_user_agent() {
    let claude = serde_json::to_vec(&serde_json::json!({"metadata":{"user_id":
        "user_ab12_account_3f2504e0-4f89-11d3-9a0c-0305e82c3301_session_6ba7b810-9dad-11d1-80b4-00c04fd430c8"}}))
        .unwrap();
    let mut headers = http::HeaderMap::new();
    headers.insert("user-agent", "claude-cli/2.0".parse().unwrap());
    headers.insert("x-opencode-session", "ses_123".parse().unwrap());
    assert_eq!(
        native_session(None, &headers, &claude).unwrap().host,
        EvaluatorClient::ClaudeCode,
        "on the secret path the body marker decides"
    );
    let bound = native_session(Some(&host("opencode")), &headers, &claude).unwrap();
    assert_eq!(bound.host, EvaluatorClient::OpenCode);
    assert_eq!(
        bound.id.as_str(),
        systemprompt_bridge::feedback::opencode_session::session_uuid("ses_123")
            .unwrap()
            .as_str(),
        "the session is read in the attested host's own shape"
    );
    assert_eq!(
        native_session(Some(&host("codex-cli")), &headers, &claude).map(|s| s.host),
        None,
        "an attested host whose session shape is absent binds nothing rather than guessing"
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
fn opencode_session_header_binds_the_uuid_or_maps_a_raw_native_id_onto_it() {
    let mapped = systemprompt_bridge::feedback::opencode_session::session_uuid("ses_x").unwrap();

    let mut current = http::HeaderMap::new();
    current.insert("user-agent", "opencode/1.0".parse().unwrap());
    current.insert("x-opencode-session", mapped.as_str().parse().unwrap());
    let session = native_session(None, &current, b"{}").unwrap();
    assert_eq!(session.host, EvaluatorClient::OpenCode);
    assert_eq!(session.id.as_str(), mapped.as_str());

    let mut legacy = http::HeaderMap::new();
    legacy.insert("user-agent", "opencode/1.0".parse().unwrap());
    legacy.insert("x-opencode-session", "ses_x".parse().unwrap());
    let session = native_session(None, &legacy, b"{}").unwrap();
    assert_eq!(session.host, EvaluatorClient::OpenCode);
    assert_eq!(
        session.id.as_str(),
        mapped.as_str(),
        "a pre-mapping plugin's raw id lands on the same session"
    );

    let mut upper = http::HeaderMap::new();
    upper.insert("user-agent", "opencode/1.0".parse().unwrap());
    upper.insert(
        "x-opencode-session",
        mapped.as_str().to_ascii_uppercase().parse().unwrap(),
    );
    assert_ne!(
        native_session(None, &upper, b"{}").unwrap().id.as_str(),
        mapped.as_str(),
        "only the canonical lowercase form is accepted as the uuid itself"
    );
}

#[test]
fn forwarded_hook_uses_protected_device_credential_and_strips_caller_credential() {
    let dir = tempfile::tempdir().unwrap();
    temp_env::with_var("XDG_STATE_HOME", Some(dir.path()), || {
        let enrollment = Enrollment::new(
            "https://example.invalid",
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

fn snapshot(path: &std::path::Path) -> (Vec<u8>, std::time::SystemTime) {
    (
        std::fs::read(path).unwrap(),
        std::fs::metadata(path).unwrap().modified().unwrap(),
    )
}

#[test]
fn a_repeated_session_and_every_read_leave_the_outbox_file_untouched() {
    let (dir, receipt) = prepared(EvaluatorClient::Codex);
    let path = dir.path().join("outbox.json");
    let outbox = Outbox::new(path.clone(), scope("device"));
    outbox.enqueue(receipt).unwrap();
    outbox
        .queue_session(EvaluatorClient::Codex, "session")
        .unwrap();
    let before = snapshot(&path);
    std::thread::sleep(std::time::Duration::from_millis(20));
    outbox
        .queue_session(EvaluatorClient::Codex, "session")
        .unwrap();
    assert_eq!(
        snapshot(&path),
        before,
        "a session already recorded is answered without a write"
    );
    outbox.entries().unwrap();
    outbox.pending_installations().unwrap();
    assert_eq!(snapshot(&path), before, "reads never rewrite the outbox");
}

#[test]
fn observed_sessions_are_deduplicated_in_memory_and_written_only_by_flush() {
    let dir = tempfile::tempdir().unwrap();
    temp_env::with_var("XDG_STATE_HOME", Some(dir.path()), || {
        let root = systemprompt_bridge::feedback::metadata_root().unwrap();
        let enrollment = Enrollment::new(
            "https://example.invalid",
            DeviceId::try_new("device").expect("nonempty fixture device"),
            UserId::new("consumer"),
            systemprompt_bridge::ids::BearerToken::new("sp_device_private"),
        )
        .unwrap();
        std::fs::create_dir_all(&root).unwrap();
        enrollment.save(&root).unwrap();
        let path = enrollment.outbox_path(&root);

        let ledger = systemprompt_bridge::feedback::sessions::NativeSessionLedger::default();
        let mut headers = http::HeaderMap::new();
        headers.insert("user-agent", "codex_cli_rs/1.0".parse().unwrap());
        headers.insert("session-id", "native-session".parse().unwrap());
        ledger.observe(None, &headers, b"{}").unwrap();
        ledger.observe(None, &headers, b"{}").unwrap();
        assert_eq!(ledger.unflushed(), 1);
        assert!(!path.exists(), "observation is not a write");

        ledger.flush("https://example.invalid").unwrap();
        assert_eq!(ledger.unflushed(), 0);
        let before = snapshot(&path);
        std::thread::sleep(std::time::Duration::from_millis(20));

        ledger.observe(None, &headers, b"{}").unwrap();
        ledger.flush("https://example.invalid").unwrap();
        assert_eq!(
            snapshot(&path),
            before,
            "a second observation of the same session does not rewrite the file"
        );
        assert!(matches!(
            ledger.flush("https://other.invalid"),
            Ok(()) | Err(FeedbackError::Scope)
        ));
    });
}

#[test]
fn a_flush_that_cannot_reach_the_outbox_keeps_the_session_for_the_next_tick() {
    let dir = tempfile::tempdir().unwrap();
    temp_env::with_var("XDG_STATE_HOME", Some(dir.path()), || {
        let ledger = systemprompt_bridge::feedback::sessions::NativeSessionLedger::default();
        let mut headers = http::HeaderMap::new();
        headers.insert("user-agent", "codex_cli_rs/1.0".parse().unwrap());
        headers.insert("session-id", "native-session".parse().unwrap());
        ledger.observe(None, &headers, b"{}").unwrap();
        assert!(matches!(
            ledger.flush("https://example.invalid"),
            Err(FeedbackError::EnrollmentRequired)
        ));
        assert_eq!(ledger.unflushed(), 1);
    });
}

#[test]
fn a_non_retryable_rejection_is_terminal_and_evictable() {
    let (dir, receipt) = prepared(EvaluatorClient::Codex);
    let outbox = Outbox::new(dir.path().join("outbox.json"), scope("device"));
    let key = outbox.enqueue(receipt.clone()).unwrap();
    outbox.delivery(&key, Err(422)).unwrap();
    let (_, entry) = outbox.entries().unwrap().into_iter().next().unwrap();
    assert!(matches!(entry.delivery, Delivery::Rejected(422)));
    outbox.delivery(&key, Err(500)).unwrap();
    let (_, entry) = outbox.entries().unwrap().into_iter().next().unwrap();
    assert!(matches!(entry.delivery, Delivery::Unacknowledged));
    outbox.delivery(&key, Err(404)).unwrap();
    for generation in 2..=512 {
        let mut next = receipt.clone();
        next.generation = generation;
        next.publication_id = PublicationId::new(format!("p-{generation}"));
        outbox.enqueue(next).unwrap();
    }
    let mut overflow = receipt;
    overflow.generation = 513;
    overflow.publication_id = PublicationId::new("overflow");
    outbox
        .enqueue(overflow)
        .expect("a rejected receipt is evicted to make room");
    assert!(
        outbox
            .entries()
            .unwrap()
            .iter()
            .all(|(_, entry)| !matches!(entry.delivery, Delivery::Rejected(_)))
    );
}
