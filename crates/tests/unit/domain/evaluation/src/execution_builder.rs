use systemprompt_evaluation::experiments::ClientKind;
use systemprompt_evaluation::experiments::execution::{
    ClientCapabilities, ExecutionEvidence, ExecutionEvidenceBuilder,
};
use systemprompt_identifiers::EvalExecutionId;

fn evidence() -> ExecutionEvidenceBuilder {
    let digest = "a".repeat(64);
    ExecutionEvidence::builder()
        .execution_id(EvalExecutionId::generate())
        .fencing_token(1)
        .capabilities(ClientCapabilities {
            client: ClientKind::ClaudeCode,
            client_version: "1.0".to_owned(),
            adapter_version: "1.0".to_owned(),
            image_digest: digest.clone(),
            supports_session_resume: false,
        })
        .installed_bundle_digest(digest.clone())
        .candidate_bundle_digest(digest.clone())
        .workspace_digest(digest)
        .requests(Vec::new())
        .artifacts(Vec::new())
        .elapsed_milliseconds(10)
        .cleanup_confirmed(true)
}

#[test]
fn evidence_requires_an_explicit_exit_outcome() {
    let error = evidence().build().expect_err("missing exit outcome");
    assert!(error.to_string().contains("exit_code is required"));
}

#[test]
fn evidence_preserves_unknown_and_known_exit_outcomes() {
    for exit_code in [None, Some(0), Some(1)] {
        let result = evidence().exit_code(exit_code).build().expect("evidence");
        assert_eq!(result.exit_code, exit_code);
    }
}
