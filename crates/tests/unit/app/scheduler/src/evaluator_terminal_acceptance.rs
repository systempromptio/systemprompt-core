//! Terminal acceptance covers actual persistence without native admission
//! proofs.
use super::*;

#[tokio::test]
async fn successful_terminal_disables_execution_credentials_and_retains_immutable_evidence() {
    let f = Fixture::new().await;
    let access = f
        .repositories
        .capabilities
        .issue(&f.owner, &f.lease)
        .await
        .unwrap();
    assert!(
        f.repositories
            .capabilities
            .authenticate(access.expose_token(), "terminal-fixture")
            .await
            .is_ok()
    );
    let cleanup = f
        .terminal()
        .cleanup(&f.owner, &f.lease, resources(), || {
            std::fs::remove_dir_all(f.root.path().join("workspace"))?;
            Ok(())
        })
        .await
        .unwrap();
    let evidence = f.evidence(true);
    assert_eq!(
        f.terminal()
            .persist(
                &f.owner,
                &f.lease,
                TerminalEvidence {
                    evidence: &evidence,
                    archive: &archive(),
                    cleanup: &cleanup
                },
                NativeCompletion::Completed
            )
            .await
            .unwrap(),
        TerminalOutcome::Completed
    );
    assert!(!f.root.path().join("workspace").exists());
    let state = f.state().await;
    assert_eq!(state["status"], "completed");
    assert_eq!(state["evidence"], 1);
    assert_eq!(state["cleanup"]["status"], "verified");
    assert_eq!(
        state["measurements"], 0,
        "terminal export alone cannot invent a judged measurement"
    );
    assert!(
        f.repositories
            .capabilities
            .authenticate(access.expose_token(), "terminal-fixture")
            .await
            .is_err()
    );
    let mut replacement = evidence.clone();
    replacement.elapsed_milliseconds += 1;
    assert!(
        f.terminal()
            .persist(
                &f.owner,
                &f.lease,
                TerminalEvidence {
                    evidence: &replacement,
                    archive: &archive(),
                    cleanup: &cleanup
                },
                NativeCompletion::Completed
            )
            .await
            .is_err()
    );
    let retained = f
        .repositories
        .evidence
        .get(&f.owner, &f.lease.execution_id)
        .await
        .unwrap();
    assert_eq!(retained.elapsed_milliseconds, evidence.elapsed_milliseconds);
    assert_eq!(f.state().await, state);
    assert_eq!(f.budget().await, (400, 0));
    f.repositories
        .lifecycle
        .reconcile_restart(&f.owner)
        .await
        .unwrap();
    assert_eq!(
        f.budget().await,
        (0, 400),
        "unsettled audit requests retain their conservative spend bound"
    );
    f.cleanup_rows().await;
}

#[tokio::test]
async fn zero_exit_without_native_completion_and_nonzero_exit_cannot_export_success() {
    for (completion, exit_code) in [
        (NativeCompletion::Incomplete, Some(0)),
        (NativeCompletion::Failed, Some(0)),
        (NativeCompletion::Completed, Some(2)),
        (NativeCompletion::Completed, None),
    ] {
        let f = Fixture::new().await;
        let cleanup = f
            .terminal()
            .cleanup(&f.owner, &f.lease, resources(), || {
                std::fs::remove_dir_all(f.root.path().join("workspace"))?;
                Ok(())
            })
            .await
            .unwrap();
        let mut evidence = f.evidence(true);
        evidence.exit_code = exit_code;
        assert_eq!(
            f.terminal()
                .persist(
                    &f.owner,
                    &f.lease,
                    TerminalEvidence {
                        evidence: &evidence,
                        archive: &archive(),
                        cleanup: &cleanup
                    },
                    completion
                )
                .await
                .unwrap(),
            TerminalOutcome::Error
        );
        let state = f.state().await;
        assert_eq!(state["status"], "error");
        assert_eq!(state["evidence"], 1);
        assert_eq!(state["measurements"], 0);
        assert_eq!(state["suggestions"], 0);
        assert_eq!(f.budget().await, (400, 0));
        f.repositories
            .lifecycle
            .reconcile_restart(&f.owner)
            .await
            .unwrap();
        assert_eq!(f.budget().await, (0, 400));
        f.cleanup_rows().await;
    }
}

#[tokio::test]
async fn mismatched_frozen_workspace_or_missing_audit_request_rejects_before_terminal_write() {
    let f = Fixture::new().await;
    let cleanup = f
        .terminal()
        .cleanup(&f.owner, &f.lease, resources(), || Ok(()))
        .await
        .unwrap();
    let before = f.state().await;
    for field in ["workspace", "installed", "candidate", "requests"] {
        let mut evidence = f.evidence(true);
        match field {
            "workspace" => evidence.workspace_digest = "e".repeat(64),
            "installed" => evidence.installed_bundle_digest = "e".repeat(64),
            "candidate" => evidence.candidate_bundle_digest = "e".repeat(64),
            _ => evidence.requests.clear(),
        }
        assert!(
            f.terminal()
                .persist(
                    &f.owner,
                    &f.lease,
                    TerminalEvidence {
                        evidence: &evidence,
                        archive: &archive(),
                        cleanup: &cleanup
                    },
                    NativeCompletion::Completed
                )
                .await
                .is_err(),
            "mismatched {field} must not be persisted"
        );
        assert_eq!(f.state().await, before);
        assert_eq!(f.budget().await, (400, 0));
    }
    f.cleanup_rows().await;
}

#[tokio::test]
async fn cleanup_witness_cannot_cross_execution_owner_boundaries() {
    let first = Fixture::new().await;
    let second = Fixture::new().await;
    let witness = first
        .terminal()
        .cleanup(&first.owner, &first.lease, resources(), || Ok(()))
        .await
        .unwrap();
    let before = second.state().await;
    assert!(
        second
            .terminal()
            .persist(
                &second.owner,
                &second.lease,
                TerminalEvidence {
                    evidence: &second.evidence(true),
                    archive: &archive(),
                    cleanup: &witness
                },
                NativeCompletion::Completed
            )
            .await
            .is_err()
    );
    assert!(
        second
            .terminal()
            .persist_blocked(
                &second.owner,
                &second.lease,
                TerminalEvidence {
                    evidence: &second.evidence(true),
                    archive: &archive(),
                    cleanup: &witness
                },
                "foreign cleanup claim"
            )
            .await
            .is_err()
    );
    assert!(
        second
            .terminal()
            .block(
                &second.owner,
                &second.lease,
                &witness,
                "foreign cleanup claim"
            )
            .await
            .is_err()
    );
    assert_eq!(second.state().await, before);
    assert_eq!(second.budget().await, (400, 0));
    assert!(second.root.path().join("workspace").exists());
    first.cleanup_rows().await;
    second.cleanup_rows().await;
}
