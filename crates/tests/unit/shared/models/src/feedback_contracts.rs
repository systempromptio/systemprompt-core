use chrono::Utc;
use systemprompt_identifiers::{
    ConsumerInstallationId, ManagedResourceId, PublicationId, ResourceRevisionId,
};
use systemprompt_models::bridge::host::HostKind;
use systemprompt_models::feedback::receipts::{
    ConsumerReceiptRequest, FileReadback, ReadbackStatus,
};
use systemprompt_models::feedback::{ContentDigest, EvaluatorClient};

#[test]
fn an_executable_unavailable_mode_or_empty_or_duplicate_readback_cannot_pass() {
    let mut receipt = ConsumerReceiptRequest {
        installation_id: ConsumerInstallationId::generate(),
        publication_id: PublicationId::generate(),
        resource_id: ManagedResourceId::generate(),
        revision_id: ResourceRevisionId::new("revision"),
        generation: 1,
        bundle_digest: ContentDigest::of(b"bundle"),
        host: EvaluatorClient::ClaudeCode,
        observed_at: Utc::now(),
        files: vec![],
        runtime_files: vec![
            systemprompt_models::feedback::receipts::RuntimeFileReadback {
                path: "SKILL.md".to_owned(),
                digest: ContentDigest::of(b"content"),
                bytes: 7,
                executable: false,
                content_check: ReadbackStatus::Verified,
                mode_check: ReadbackStatus::Verified,
            },
        ],
    };
    assert!(!receipt.fully_verified());
    receipt.files.push(FileReadback {
        revision_id: receipt.revision_id.clone(),
        path: "SKILL.md".to_owned(),
        digest: ContentDigest::of(b"content"),
        bytes: 7,
        executable: false,
        content_check: ReadbackStatus::Verified,
        mode_check: ReadbackStatus::Unavailable,
    });
    // A plain file has no mode to satisfy, so a host without POSIX mode bits
    // reporting it unavailable leaves nothing unchecked.
    assert!(receipt.fully_verified());

    // An executable still needs its bit confirmed: the same unavailable mode
    // is now an unverified file, which is what a Windows host must not pass.
    receipt.files[0].executable = true;
    assert!(!receipt.fully_verified());
    receipt.files[0].mode_check = ReadbackStatus::Verified;
    assert!(receipt.fully_verified());

    receipt.files[0].content_check = ReadbackStatus::Unavailable;
    assert!(!receipt.fully_verified());
    receipt.files[0].content_check = ReadbackStatus::Verified;

    receipt.files.push(receipt.files[0].clone());
    assert!(!receipt.fully_verified());
}

#[test]
fn the_evaluator_client_wire_is_kebab_case_with_no_alias() {
    assert_eq!(
        serde_json::to_string(&EvaluatorClient::OpenCode).unwrap(),
        "\"open-code\""
    );
    assert!(serde_json::from_str::<EvaluatorClient>("\"opencode\"").is_err());
    assert!(serde_json::from_str::<ContentDigest>("\"not-a-digest\"").is_err());
}

#[test]
fn an_evaluator_client_accepts_both_packagings_of_its_product() {
    assert!(EvaluatorClient::Codex.accepts(HostKind::CodexCli));
    assert!(EvaluatorClient::Codex.accepts(HostKind::Codex));
    assert!(EvaluatorClient::OpenCode.accepts(HostKind::OpenCode));
    assert!(EvaluatorClient::OpenCode.accepts(HostKind::OpenCodeCli));
    assert!(!EvaluatorClient::Codex.accepts(HostKind::Hermes));
}
