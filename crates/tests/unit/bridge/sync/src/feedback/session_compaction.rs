use super::*;

fn acknowledge_installation(outbox: &Outbox, key: &str, receipt: &str) {
    outbox
        .delivery(
            key,
            Ok(ConsumerReceiptResponse {
                receipt_id: InstallationReceiptId::new(receipt),
                acknowledgement: ReceiptAcknowledgement::Accepted,
                acknowledged_at: Utc::now(),
                fully_verified: true,
            }),
        )
        .unwrap();
}

#[test]
fn completed_sessions_release_both_capacity_bounds_without_changing_receipt() {
    let (dir, receipt) = prepared(EvaluatorClient::Codex);
    let outbox = Outbox::new(dir.path().join("outbox.json"), scope("device"));
    let key = outbox.enqueue(receipt.clone()).unwrap();
    acknowledge_installation(&outbox, &key, "receipt");
    for index in 0..1025 {
        let session = format!("session-{index:04}");
        outbox
            .queue_session(EvaluatorClient::Codex, &session)
            .unwrap();
        outbox
            .acknowledge_session(&key, &InstallationReceiptId::new("receipt"), &session)
            .unwrap();
        assert!(outbox.entries().unwrap()[0].1.session_bindings.is_empty());
    }
    let persisted: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.path().join("outbox.json")).unwrap()).unwrap();
    assert_eq!(persisted["sessions"].as_object().unwrap().len(), 1024);
    assert_eq!(outbox.enqueue(receipt.clone()).unwrap(), key);
    let mut conflict = receipt;
    conflict.runtime_files[0].digest = ContentDigest::of(b"tampered");
    assert!(matches!(
        outbox.enqueue(conflict),
        Err(FeedbackError::Readback(ReadbackFault::AcknowledgedConflict))
    ));
}

#[test]
fn pending_capacity_rejects_atomically_and_restart_preserves_mixed_delivery() {
    let (dir, receipt) = prepared(EvaluatorClient::Codex);
    let path = dir.path().join("outbox.json");
    let outbox = Outbox::new(path.clone(), scope("device"));
    let key = outbox.enqueue(receipt).unwrap();
    acknowledge_installation(&outbox, &key, "receipt");
    for index in 0..256 {
        outbox
            .queue_session(EvaluatorClient::Codex, &format!("pending-{index}"))
            .unwrap();
    }
    let before = std::fs::read(&path).unwrap();
    assert!(matches!(
        outbox.queue_session(EvaluatorClient::Codex, "overflow"),
        Err(FeedbackError::Full)
    ));
    assert_eq!(std::fs::read(&path).unwrap(), before);
    outbox
        .acknowledge_session(&key, &InstallationReceiptId::new("receipt"), "pending-0")
        .unwrap();
    let recovered = Outbox::new(path, scope("device"));
    recovered
        .queue_session(EvaluatorClient::Codex, "replacement")
        .unwrap();
    let entries = recovered.entries().unwrap();
    assert_eq!(entries[0].1.session_bindings.len(), 256);
    assert!(!entries[0].1.session_bindings.contains_key("pending-0"));
    assert!(entries[0].1.session_bindings.values().all(|bound| !bound));
    for index in 1..256 {
        assert_eq!(
            entries[0]
                .1
                .session_bindings
                .get(&format!("pending-{index}")),
            Some(&false)
        );
    }
}

#[test]
fn acknowledged_retry_remains_frozen_across_restart_and_upgrade() {
    let (dir, receipt) = prepared(EvaluatorClient::Codex);
    let path = dir.path().join("outbox.json");
    let outbox = Outbox::new(path.clone(), scope("device"));
    let key = outbox.enqueue(receipt.clone()).unwrap();
    acknowledge_installation(&outbox, &key, "receipt");
    outbox
        .queue_session(EvaluatorClient::Codex, "original")
        .unwrap();
    outbox
        .acknowledge_session(&key, &InstallationReceiptId::new("receipt"), "original")
        .unwrap();
    let recovered = Outbox::new(path, scope("device"));
    recovered
        .acknowledge_session(&key, &InstallationReceiptId::new("receipt"), "original")
        .unwrap();
    assert!(matches!(
        recovered.acknowledge_session(&key, &InstallationReceiptId::new("forged"), "original"),
        Err(FeedbackError::Scope)
    ));
    let mut upgrade = receipt;
    upgrade.generation = 2;
    upgrade.publication_id = PublicationId::new("upgrade");
    recovered.enqueue(upgrade).unwrap();
    recovered
        .queue_session(EvaluatorClient::Codex, "original")
        .unwrap();
    assert!(
        recovered
            .entries()
            .unwrap()
            .iter()
            .all(|(_, entry)| entry.session_bindings.is_empty())
    );
}

#[test]
fn one_acknowledged_resource_cannot_discard_another_resources_pending_binding() {
    let (dir, receipt) = prepared(EvaluatorClient::Codex);
    let path = dir.path().join("outbox.json");
    let outbox = Outbox::new(path.clone(), scope("device"));
    let first = outbox.enqueue(receipt.clone()).unwrap();
    let mut second_receipt = receipt;
    second_receipt.resource_id = ManagedResourceId::new("second-resource");
    second_receipt.publication_id = PublicationId::new("second-publication");
    let second = outbox.enqueue(second_receipt).unwrap();
    acknowledge_installation(&outbox, &first, "first");
    acknowledge_installation(&outbox, &second, "second");
    outbox
        .queue_session(EvaluatorClient::Codex, "shared")
        .unwrap();
    outbox
        .acknowledge_session(&first, &InstallationReceiptId::new("first"), "shared")
        .unwrap();
    let recovered = Outbox::new(path, scope("device"));
    for (key, entry) in recovered.entries().unwrap() {
        assert_eq!(entry.session_bindings.get("shared"), Some(&(key == first)));
    }
    recovered
        .acknowledge_session(&second, &InstallationReceiptId::new("second"), "shared")
        .unwrap();
    assert!(
        recovered
            .entries()
            .unwrap()
            .iter()
            .all(|(_, entry)| entry.session_bindings.is_empty())
    );
}

#[test]
fn legacy_completed_bindings_compact_without_losing_pending_installation_sessions() {
    let (dir, receipt) = prepared(EvaluatorClient::Codex);
    let path = dir.path().join("outbox.json");
    let outbox = Outbox::new(path.clone(), scope("device"));
    let key = outbox.enqueue(receipt).unwrap();
    acknowledge_installation(&outbox, &key, "receipt");
    outbox
        .queue_session(EvaluatorClient::Codex, "legacy-completed")
        .unwrap();
    let mut state: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    state.as_object_mut().unwrap().remove("completed_sessions");
    state["entries"][&key]["session_bindings"]["legacy-completed"] = serde_json::json!(true);
    std::fs::write(&path, serde_json::to_vec(&state).unwrap()).unwrap();
    let recovered = Outbox::new(path, scope("device"));
    let mut pending = publication();
    pending.resource_id = ManagedResourceId::new("pending-resource");
    pending.publication_id = PublicationId::new("pending-publication");
    recovered
        .reserve_installation(
            systemprompt_bridge::feedback::outbox::PendingInstallation::new(
                pending,
                EvaluatorClient::Codex,
                vec![dir.path().join("pending")],
            ),
        )
        .unwrap();
    recovered
        .queue_session(EvaluatorClient::Codex, "needs-installation")
        .unwrap();
    recovered
        .acknowledge_session(
            &key,
            &InstallationReceiptId::new("receipt"),
            "needs-installation",
        )
        .unwrap();
    let entries = recovered.entries().unwrap();
    assert!(
        !entries[0]
            .1
            .session_bindings
            .contains_key("legacy-completed")
    );
    assert_eq!(
        entries[0].1.session_bindings.get("needs-installation"),
        Some(&true)
    );
    assert_eq!(recovered.pending_installations().unwrap().len(), 1);
}
