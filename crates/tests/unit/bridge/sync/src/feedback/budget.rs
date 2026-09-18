use super::*;
use systemprompt_bridge::feedback::outbox::PendingInstallation;
use systemprompt_bridge::feedback::{RecoveryProgress, recover_pending};

fn manifest_for(host: &str) -> systemprompt_bridge::gateway::manifest::SignedManifest {
    serde_json::from_value(serde_json::json!({
        "min_schema_version": 1, "manifest_version": "2026-04-30T12:00:00Z-deadbeef",
        "issued_at":"2026-04-30T12:00:00Z", "not_before":"2026-04-30T12:00:00Z",
        "user_id":"consumer", "plugins":[], "managed_mcp_servers":[], "revocations":[],
        "enabled_hosts":[host],
        "skills":[{"id":"skill", "name":"Skill", "description":"", "tags":[], "file_path":"skill/SKILL.md",
                   "sha256":"0".repeat(64), "instructions":"", "publication":publication()}]
    }))
    .unwrap()
}

fn enrollment_for(gateway: &str) -> Enrollment {
    Enrollment::new(
        gateway,
        DeviceId::try_new("device").expect("nonempty fixture device"),
        UserId::new("consumer"),
        systemprompt_bridge::ids::BearerToken::new("sp_device_private"),
    )
    .unwrap()
}

// An exhausted budget is reported as pending work, not spent: nothing is
// attempted (no backoff is charged) and nothing is fabricated.
#[test]
fn an_exhausted_budget_reports_the_due_work_as_remaining_without_an_attempt() {
    let dir = tempfile::tempdir().unwrap();
    temp_env::with_var("XDG_STATE_HOME", Some(dir.path()), || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let enrollment = enrollment_for("http://127.0.0.1:9");
                let outbox = Outbox::new(
                    enrollment.outbox_path(dir.path()),
                    OutboxScope::from_enrollment(&enrollment),
                );
                outbox
                    .reserve_installation(PendingInstallation::new(
                        publication(),
                        EvaluatorClient::Codex,
                        vec![dir.path().to_path_buf()],
                    ))
                    .unwrap();
                let progress = recover_pending(
                    &enrollment,
                    &outbox,
                    EvaluatorClient::Codex,
                    &manifest_for("codex-cli"),
                    std::time::Instant::now(),
                )
                .await
                .unwrap();
                assert_eq!(
                    progress,
                    RecoveryProgress {
                        recovered: 0,
                        remaining: 1
                    }
                );
                let pending = outbox.pending_installations().unwrap();
                assert_eq!(pending.len(), 1);
                assert_eq!(pending[0].1.attempts, 0, "no attempt was charged");
                assert!(
                    outbox.entries().unwrap().is_empty(),
                    "no receipt was fabricated"
                );
            });
    });
}

// A gateway slower than the remaining budget: the pass stops at the deadline
// and says how much is left, instead of an `Elapsed` that reads as a fault.
#[test]
fn a_slow_gateway_ends_the_pass_at_the_deadline_with_the_remainder_counted() {
    let dir = tempfile::tempdir().unwrap();
    temp_env::with_var("XDG_STATE_HOME", Some(dir.path()), || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
                let gateway = format!("http://{}", listener.local_addr().unwrap());
                let stall = std::thread::spawn(move || {
                    let (stream, _) = listener.accept().unwrap();
                    std::thread::sleep(std::time::Duration::from_secs(6));
                    drop(stream);
                });
                let enrollment = enrollment_for(&gateway);
                let outbox = Outbox::new(
                    enrollment.outbox_path(dir.path()),
                    OutboxScope::from_enrollment(&enrollment),
                );
                outbox
                    .reserve_installation(PendingInstallation::new(
                        publication(),
                        EvaluatorClient::Codex,
                        vec![dir.path().to_path_buf()],
                    ))
                    .unwrap();
                let started = std::time::Instant::now();
                let progress = recover_pending(
                    &enrollment,
                    &outbox,
                    EvaluatorClient::Codex,
                    &manifest_for("codex-cli"),
                    started + std::time::Duration::from_secs(3),
                )
                .await
                .unwrap();
                let elapsed = started.elapsed();
                assert!(
                    elapsed >= std::time::Duration::from_secs(3)
                        && elapsed < std::time::Duration::from_secs(5),
                    "the pass ended at its deadline, took {elapsed:?}"
                );
                assert_eq!(
                    progress,
                    RecoveryProgress {
                        recovered: 0,
                        remaining: 1
                    }
                );
                assert_eq!(outbox.pending_installations().unwrap()[0].1.attempts, 1);
                stall.join().unwrap();
            });
    });
}

// A receipt the gateway already acknowledged for this exact publication is
// evidence on file; the capture skips it rather than re-planning it on every
// sync (one bundle round-trip per skill per host, per sync).
#[test]
fn an_acknowledged_receipt_for_the_same_publication_is_evidence_on_file() {
    let (dir, receipt) = prepared(EvaluatorClient::Codex);
    let outbox = Outbox::new(dir.path().join("outbox.json"), scope("device"));
    let key = outbox.enqueue(receipt).unwrap();
    assert!(
        !outbox
            .has_acknowledged_receipt(EvaluatorClient::Codex, &publication())
            .unwrap(),
        "queued is not acknowledged"
    );
    outbox
        .delivery(
            &key,
            Ok(ConsumerReceiptResponse {
                receipt_id: InstallationReceiptId::new("receipt"),
                acknowledgement: ReceiptAcknowledgement::Accepted,
                acknowledged_at: Utc::now(),
                fully_verified: true,
            }),
        )
        .unwrap();
    assert!(
        outbox
            .has_acknowledged_receipt(EvaluatorClient::Codex, &publication())
            .unwrap()
    );
    assert!(
        !outbox
            .has_acknowledged_receipt(EvaluatorClient::ClaudeCode, &publication())
            .unwrap(),
        "another host's receipt is not this host's"
    );
    let mut newer = publication();
    newer.bundle_digest = systemprompt_bridge::ids::Sha256Digest::try_new("1".repeat(64)).unwrap();
    assert!(
        !outbox
            .has_acknowledged_receipt(EvaluatorClient::Codex, &newer)
            .unwrap(),
        "a new bundle digest needs new evidence"
    );
}
