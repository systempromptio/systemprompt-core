//! Unit tests for the experiment and execution builders in
//! `crates/domain/evaluation/src/experiments/{builder,execution_builders}.rs`.

use systemprompt_evaluation::experiments::execution::{
    ArtifactEvidence, ClientCapabilities, ClientCapabilitiesBuilder, ExecutionEvidence,
    ExecutionLimits, ExecutionLimitsBuilder,
};
use systemprompt_evaluation::experiments::{
    ClientKind, ExecutionMode, ExperimentSpec, Objective, VariantSpec,
};
use systemprompt_identifiers::{
    AiRequestId, EvalExecutionId, EvalRevisionId, ModelId, ProviderId,
};

fn digest_of(byte: char) -> String {
    std::iter::repeat_n(byte, 64).collect()
}

fn variant(client_version: &str) -> VariantSpec {
    VariantSpec {
        client: ClientKind::ClaudeCode,
        client_version: client_version.to_owned(),
        model: ModelId::new("claude-opus-5"),
        provider: ProviderId::new("anthropic"),
        skill_bundle_digest: digest_of('a'),
        configuration_digest: digest_of('b'),
        worker_image_digest: digest_of('c'),
    }
}

fn limits() -> ExecutionLimitsBuilder {
    ExecutionLimits::builder()
        .max_turns(4)
        .max_output_tokens(1024)
        .active_timeout_seconds(60)
        .max_artifact_bytes(1024)
}

fn capabilities() -> ClientCapabilitiesBuilder {
    ClientCapabilities::builder()
        .client(ClientKind::ClaudeCode)
        .client_version("1.0.0".to_owned())
        .adapter_version("2.0.0".to_owned())
        .image_digest(digest_of('d'))
        .supports_session_resume(false)
}

#[test]
fn experiment_builder_defaults_are_overridden_by_each_setter() {
    let spec = ExperimentSpec::builder("nightly", EvalRevisionId::new("rubric-1"))
        .cases(vec![EvalRevisionId::new("case-1")])
        .variants(vec![variant("pinned")])
        .budget_microdollars(1)
        .build()
        .expect("defaults build");
    assert_eq!(spec.schema_version, 1);
    assert_eq!(spec.name, "nightly");
    assert_eq!(spec.repetitions, 1);
    assert_eq!(spec.execution_mode, ExecutionMode::Fixture);
    assert_eq!(spec.objective, Objective::Quality);
    assert_eq!(spec.budget_microdollars, 1);

    let tuned = ExperimentSpec::builder("tuned", EvalRevisionId::new("rubric-1"))
        .cases(vec![EvalRevisionId::new("case-1")])
        .variants(vec![variant("pinned"), variant("candidate")])
        .repetitions(3)
        .budget_microdollars(9_000)
        .execution_mode(ExecutionMode::Live)
        .objective(Objective::Cost)
        .build()
        .expect("tuned build");
    assert_eq!(tuned.repetitions, 3);
    assert_eq!(tuned.budget_microdollars, 9_000);
    assert_eq!(tuned.execution_mode, ExecutionMode::Live);
    assert_eq!(tuned.objective, Objective::Cost);
    assert_eq!(tuned.variants.len(), 2);
}

#[test]
fn experiment_builder_refuses_an_empty_or_unbounded_matrix() {
    let no_cases = ExperimentSpec::builder("empty", EvalRevisionId::new("rubric-1"))
        .variants(vec![variant("pinned")])
        .budget_microdollars(1)
        .build()
        .expect_err("no cases");
    assert!(no_cases.to_string().contains("1–100 cases"));

    let no_variants = ExperimentSpec::builder("empty", EvalRevisionId::new("rubric-1"))
        .cases(vec![EvalRevisionId::new("case-1")])
        .budget_microdollars(1)
        .build()
        .expect_err("no variants");
    assert!(no_variants.to_string().contains("1–16 variants"));

    let too_many_repetitions = ExperimentSpec::builder("wide", EvalRevisionId::new("rubric-1"))
        .cases(vec![EvalRevisionId::new("case-1")])
        .variants(vec![variant("pinned")])
        .repetitions(11)
        .budget_microdollars(1)
        .build()
        .expect_err("repetitions");
    assert!(too_many_repetitions.to_string().contains("repetitions"));

    let unnamed = ExperimentSpec::builder("   ", EvalRevisionId::new("rubric-1"))
        .cases(vec![EvalRevisionId::new("case-1")])
        .variants(vec![variant("pinned")])
        .budget_microdollars(1)
        .build()
        .expect_err("blank name");
    assert!(unnamed.to_string().contains("schema version 1"));

    let unfunded = ExperimentSpec::builder("free", EvalRevisionId::new("rubric-1"))
        .cases(vec![EvalRevisionId::new("case-1")])
        .variants(vec![variant("pinned")])
        .budget_microdollars(0)
        .build()
        .expect_err("no budget");
    assert!(unfunded.to_string().contains("positive budget"));
}

#[test]
fn execution_limits_builder_names_each_missing_field() {
    limits().build().expect("complete limits");

    let cases: [(&str, ExecutionLimitsBuilder); 4] = [
        (
            "max_turns is required",
            ExecutionLimits::builder()
                .max_output_tokens(1024)
                .active_timeout_seconds(60)
                .max_artifact_bytes(1024),
        ),
        (
            "max_output_tokens is required",
            ExecutionLimits::builder()
                .max_turns(4)
                .active_timeout_seconds(60)
                .max_artifact_bytes(1024),
        ),
        (
            "active_timeout_seconds is required",
            ExecutionLimits::builder()
                .max_turns(4)
                .max_output_tokens(1024)
                .max_artifact_bytes(1024),
        ),
        (
            "max_artifact_bytes is required",
            ExecutionLimits::builder()
                .max_turns(4)
                .max_output_tokens(1024)
                .active_timeout_seconds(60),
        ),
    ];
    for (message, builder) in cases {
        let error = builder.build().expect_err(message);
        assert!(error.to_string().contains(message), "{error}");
    }
}

#[test]
fn execution_limits_builder_applies_envelope_validation() {
    let error = limits()
        .max_turns(500)
        .build()
        .expect_err("envelope enforced after assembly");
    assert!(error.to_string().contains("supported envelope"));
}

#[test]
fn client_capabilities_builder_names_each_missing_field() {
    let built = capabilities().build().expect("complete capabilities");
    assert_eq!(built.client, ClientKind::ClaudeCode);
    assert!(!built.supports_session_resume);

    let cases: [(&str, ClientCapabilitiesBuilder); 5] = [
        (
            "client is required",
            ClientCapabilities::builder()
                .client_version("1".to_owned())
                .adapter_version("2".to_owned())
                .image_digest(digest_of('d'))
                .supports_session_resume(false),
        ),
        (
            "client_version is required",
            ClientCapabilities::builder()
                .client(ClientKind::Opencode)
                .adapter_version("2".to_owned())
                .image_digest(digest_of('d'))
                .supports_session_resume(false),
        ),
        (
            "adapter_version is required",
            ClientCapabilities::builder()
                .client(ClientKind::Opencode)
                .client_version("1".to_owned())
                .image_digest(digest_of('d'))
                .supports_session_resume(false),
        ),
        (
            "image_digest is required",
            ClientCapabilities::builder()
                .client(ClientKind::Opencode)
                .client_version("1".to_owned())
                .adapter_version("2".to_owned())
                .supports_session_resume(false),
        ),
        (
            "supports_session_resume is required",
            ClientCapabilities::builder()
                .client(ClientKind::Opencode)
                .client_version("1".to_owned())
                .adapter_version("2".to_owned())
                .image_digest(digest_of('d')),
        ),
    ];
    for (message, builder) in cases {
        let error = builder.build().expect_err(message);
        assert!(error.to_string().contains(message), "{error}");
    }
}

#[test]
fn client_capabilities_builder_applies_field_validation() {
    let error = capabilities()
        .image_digest("not-a-digest".to_owned())
        .build()
        .expect_err("digest validated");
    assert!(error.to_string().contains("lowercase SHA-256"));
}

#[test]
fn artifact_evidence_builder_names_each_missing_field() {
    let built = ArtifactEvidence::builder()
        .relative_path("out/report.md".to_owned())
        .sha256(digest_of('e'))
        .bytes(64)
        .build()
        .expect("complete artifact");
    assert_eq!(built.relative_path, "out/report.md");
    assert_eq!(built.bytes, 64);

    let missing_path = ArtifactEvidence::builder()
        .sha256(digest_of('e'))
        .bytes(64)
        .build()
        .expect_err("path");
    assert!(missing_path.to_string().contains("relative_path is required"));

    let missing_hash = ArtifactEvidence::builder()
        .relative_path("out/report.md".to_owned())
        .bytes(64)
        .build()
        .expect_err("sha256");
    assert!(missing_hash.to_string().contains("sha256 is required"));

    let missing_bytes = ArtifactEvidence::builder()
        .relative_path("out/report.md".to_owned())
        .sha256(digest_of('e'))
        .build()
        .expect_err("bytes");
    assert!(missing_bytes.to_string().contains("bytes is required"));
}

#[test]
fn execution_evidence_builder_names_each_missing_field_and_validates() {
    let complete = ExecutionEvidence::builder()
        .execution_id(EvalExecutionId::generate())
        .fencing_token(7)
        .capabilities(capabilities().build().expect("capabilities"))
        .installed_bundle_digest(digest_of('a'))
        .candidate_bundle_digest(digest_of('b'))
        .workspace_digest(digest_of('c'))
        .requests(vec![AiRequestId::generate()])
        .artifacts(Vec::new())
        .exit_code(Some(0))
        .elapsed_milliseconds(5)
        .cleanup_confirmed(true)
        .build()
        .expect("complete evidence");
    assert_eq!(complete.fencing_token, 7);
    assert_eq!(complete.requests.len(), 1);

    let missing_execution = ExecutionEvidence::builder()
        .fencing_token(7)
        .build()
        .expect_err("execution_id");
    assert!(
        missing_execution
            .to_string()
            .contains("execution_id is required")
    );

    let missing_fencing = ExecutionEvidence::builder()
        .execution_id(EvalExecutionId::generate())
        .build()
        .expect_err("fencing_token");
    assert!(
        missing_fencing
            .to_string()
            .contains("fencing_token is required")
    );

    let missing_capabilities = ExecutionEvidence::builder()
        .execution_id(EvalExecutionId::generate())
        .fencing_token(1)
        .build()
        .expect_err("capabilities");
    assert!(
        missing_capabilities
            .to_string()
            .contains("capabilities is required")
    );

    let unfenced = ExecutionEvidence::builder()
        .execution_id(EvalExecutionId::generate())
        .fencing_token(0)
        .capabilities(capabilities().build().expect("capabilities"))
        .installed_bundle_digest(digest_of('a'))
        .candidate_bundle_digest(digest_of('b'))
        .workspace_digest(digest_of('c'))
        .requests(Vec::new())
        .artifacts(Vec::new())
        .exit_code(None)
        .elapsed_milliseconds(5)
        .cleanup_confirmed(false)
        .build()
        .expect_err("validation runs after assembly");
    assert!(unfenced.to_string().contains("lease or manifest size"));
}
