use super::*;

#[test]
fn every_host_has_actual_entrypoint_and_supporting_file_readback() {
    for host in [
        EvaluatorClient::ClaudeCode,
        EvaluatorClient::OpenCode,
        EvaluatorClient::Codex,
        EvaluatorClient::Hermes,
        EvaluatorClient::ClaudeDesktop,
    ] {
        let (dir, receipt) = prepared(host);
        #[cfg(unix)]
        assert!(receipt.fully_verified());
        #[cfg(not(unix))]
        assert!(!receipt.fully_verified());
        assert!(
            receipt
                .runtime_files
                .iter()
                .any(|file| file.path == "SKILL.md")
        );
        std::fs::write(dir.path().join("scripts/run.sh"), b"echo evil").unwrap();
        assert!(
            readback::verify(
                dir.path(),
                &plan(host),
                ConsumerInstallationId::new("installation")
            )
            .is_err()
        );
        assert_eq!(
            std::fs::read(
                dir.path()
                    .join(".systemprompt-source/revision/scripts/run.sh")
            )
            .unwrap(),
            b"echo safe"
        );
    }
}

#[cfg(unix)]
#[test]
fn executable_tamper_symlink_and_partial_files_never_pass() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let (dir, _) = prepared(EvaluatorClient::Codex);
    let plan = plan(EvaluatorClient::Codex);
    let script = dir.path().join("scripts/run.sh");
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(readback::verify(dir.path(), &plan, ConsumerInstallationId::new("i")).is_err());
    std::fs::remove_file(&script).unwrap();
    assert!(readback::verify(dir.path(), &plan, ConsumerInstallationId::new("i")).is_err());
    let outside = tempfile::NamedTempFile::new().unwrap();
    symlink(outside.path(), &script).unwrap();
    assert!(readback::materialize(dir.path(), &plan).is_err());
}

#[test]
fn pending_outbox_survives_restart_and_fresh_readback_replaces_unacknowledged() {
    let (dir, receipt) = prepared(EvaluatorClient::Codex);
    let path = dir.path().join("outbox.json");
    let outbox = Outbox::new(path.clone(), scope("device"));
    let key = outbox.enqueue(receipt.clone()).unwrap();
    let recovered = Outbox::new(path, scope("device"));
    assert!(matches!(
        recovered.entries().unwrap()[0].1.delivery,
        Delivery::Unacknowledged
    ));
    let mut retry = receipt.clone();
    retry.observed_at = Utc::now();
    assert_eq!(recovered.enqueue(retry).unwrap(), key);
    let mut fresh = receipt.clone();
    fresh.runtime_files[0].digest = ContentDigest::of(b"different");
    assert_eq!(recovered.enqueue(fresh.clone()).unwrap(), key);
    assert_eq!(
        recovered.entries().unwrap()[0].1.request.runtime_files[0].digest,
        fresh.runtime_files[0].digest
    );
    recovered
        .delivery(
            &key,
            Ok(ConsumerReceiptResponse {
                receipt_id: InstallationReceiptId::new("server-receipt"),
                acknowledgement: ReceiptAcknowledgement::IdenticalRetry,
                acknowledged_at: Utc::now(),
                fully_verified: true,
            }),
        )
        .unwrap();
    assert!(matches!(
        recovered.entries().unwrap()[0].1.delivery,
        Delivery::Acknowledged(_)
    ));
    assert!(matches!(
        recovered.enqueue(receipt),
        Err(FeedbackError::Readback(ReadbackFault::AcknowledgedConflict))
    ));
}

#[test]
fn conflicted_receipt_is_replaced_by_a_fresh_readback() {
    let (dir, receipt) = prepared(EvaluatorClient::Codex);
    let outbox = Outbox::new(dir.path().join("outbox.json"), scope("device"));
    let key = outbox.enqueue(receipt.clone()).unwrap();
    outbox.delivery(&key, Err(409)).unwrap();
    let mut fresh = receipt;
    fresh.runtime_files[0].digest = ContentDigest::of(b"reinstalled");
    assert_eq!(outbox.enqueue(fresh).unwrap(), key);
    let entry = &outbox.entries().unwrap()[0].1;
    assert!(matches!(entry.delivery, Delivery::Unacknowledged));
    assert_eq!(entry.attempts, 0);
}

#[test]
fn device_or_account_change_cannot_replay_pending_evidence_but_rotation_can() {
    let (dir, receipt) = prepared(EvaluatorClient::Codex);
    let path = dir.path().join("outbox.json");
    let original = Outbox::new(path.clone(), scope("original-device"));
    original.enqueue(receipt).unwrap();
    assert!(matches!(
        Outbox::new(path.clone(), scope("different-device")).entries(),
        Err(FeedbackError::Scope)
    ));
    let mut different_user = scope("original-device");
    different_user.consumer_id = UserId::new("other-user");
    assert!(matches!(
        Outbox::new(path.clone(), different_user).entries(),
        Err(FeedbackError::Scope)
    ));
    let rotated = Outbox::new(path, scope("original-device"));
    assert_eq!(rotated.entries().unwrap().len(), 1);
    let old = Enrollment::new(
        "https://example.invalid",
        DeviceId::try_new("device").expect("nonempty fixture device"),
        UserId::new("consumer"),
        systemprompt_bridge::ids::BearerToken::new("sp_device_old"),
    )
    .unwrap();
    let new = Enrollment::new(
        "https://example.invalid",
        DeviceId::try_new("device").expect("nonempty fixture device"),
        UserId::new("consumer"),
        systemprompt_bridge::ids::BearerToken::new("sp_device_rotated"),
    )
    .unwrap();
    assert_eq!(
        OutboxScope::from_enrollment(&old),
        OutboxScope::from_enrollment(&new)
    );
    assert!(!format!("{old:?}").contains("sp_device_old"));
}

#[test]
fn conflict_and_authentication_rejection_remain_explicit_after_restart() {
    let (dir, receipt) = prepared(EvaluatorClient::Hermes);
    let path = dir.path().join("outbox.json");
    let outbox = Outbox::new(path.clone(), scope("device"));
    let key = outbox.enqueue(receipt).unwrap();
    outbox.delivery(&key, Err(401)).unwrap();
    assert!(matches!(
        Outbox::new(path.clone(), scope("device"))
            .entries()
            .unwrap()[0]
            .1
            .delivery,
        Delivery::CredentialRejected
    ));
    outbox.delivery(&key, Err(409)).unwrap();
    assert!(matches!(
        Outbox::new(path, scope("device")).entries().unwrap()[0]
            .1
            .delivery,
        Delivery::Conflict
    ));
}


#[test]
fn every_host_upgrade_and_rollback_reverify_active_bytes_and_retain_offline_generation_evidence() {
    for host in [
        EvaluatorClient::ClaudeCode,
        EvaluatorClient::OpenCode,
        EvaluatorClient::Codex,
        EvaluatorClient::Hermes,
        EvaluatorClient::ClaudeDesktop,
    ] {
        let (dir, initial) = prepared(host);
        let path = dir.path().join("outbox.json");
        let outbox = Outbox::new(path.clone(), scope("device"));
        let first = outbox.enqueue(initial).unwrap();
        let mut upgraded = plan(host);
        upgraded.generation = 2;
        upgraded.publication_id = PublicationId::new("upgrade-publication");
        upgraded.revision_id = ResourceRevisionId::new("upgraded-revision");
        upgraded.bundle_digest = ContentDigest::of(b"upgraded bundle");
        let bytes = b"echo upgraded".to_vec();
        upgraded.canonical_files[0].revision_id = upgraded.revision_id.clone();
        upgraded.canonical_files[0].digest = ContentDigest::of(&bytes);
        upgraded.canonical_files[0].bytes = bytes.len() as u64;
        upgraded.runtime_files[0].path =
            ".systemprompt-source/upgraded-revision/scripts/run.sh".to_owned();
        upgraded.runtime_files[0].bytes = bytes.clone();
        upgraded.runtime_files[1].bytes = bytes;
        assert!(
            readback::verify(
                dir.path(),
                &upgraded,
                ConsumerInstallationId::new("upgrade")
            )
            .is_err()
        );
        readback::materialize(dir.path(), &upgraded).unwrap();
        let upgrade = readback::verify(
            dir.path(),
            &upgraded,
            ConsumerInstallationId::new("upgrade"),
        )
        .unwrap();
        let second = outbox.enqueue(upgrade).unwrap();
        assert!(
            readback::verify(
                dir.path(),
                &plan(host),
                ConsumerInstallationId::new("stale")
            )
            .is_err()
        );
        let mut rollback = plan(host);
        rollback.generation = 3;
        rollback.publication_id = PublicationId::new("rollback-publication");
        readback::materialize(dir.path(), &rollback).unwrap();
        let rolled_back = readback::verify(
            dir.path(),
            &rollback,
            ConsumerInstallationId::new("rollback"),
        )
        .unwrap();
        let third = outbox.enqueue(rolled_back).unwrap();
        assert_ne!(first, second);
        assert_ne!(first, third);
        assert_ne!(second, third);
        let recovered = Outbox::new(path, scope("device"));
        let entries = recovered.entries().unwrap();
        assert_eq!(entries.len(), 3);
        assert!(
            entries
                .iter()
                .all(|(_, entry)| matches!(entry.delivery, Delivery::Unacknowledged))
        );
        assert_eq!(
            std::fs::read(dir.path().join("scripts/run.sh")).unwrap(),
            b"echo safe"
        );
    }
}
