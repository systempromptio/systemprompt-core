use systemprompt_evaluation::capabilities::{
    CapabilityAvailability, EvaluatorClient, UnsupportedCapabilityReason, VerifiedNativeTarget,
    admit_variant, evaluator_capabilities,
};
use systemprompt_evaluation::experiments::{ClientKind, VariantSpec};
use systemprompt_identifiers::{ModelId, ProviderId};

fn variant() -> VariantSpec {
    VariantSpec {
        client: ClientKind::ClaudeCode,
        client_version: "1.2.3".to_owned(),
        model: ModelId::new("fixture-model"),
        provider: ProviderId::new("anthropic"),
        skill_bundle_digest: "a".repeat(64),
        configuration_digest: "b".repeat(64),
        worker_image_digest: "c".repeat(64),
    }
}

#[test]
fn capability_discovery_and_admission_both_reject_unverified_targets() {
    for capability in evaluator_capabilities() {
        assert_eq!(
            capability.automated_evaluation,
            CapabilityAvailability::Unsupported
        );
        assert!(capability.verified_targets.is_empty());
        if capability.client == EvaluatorClient::ClaudeDesktop {
            assert!(capability.installation_supported);
            assert_eq!(
                capability.reason,
                Some(UnsupportedCapabilityReason::InteractiveHost)
            );
        }
    }
    assert!(admit_variant(&variant()).is_err());
}

#[test]
fn native_proof_requires_exact_platform_version_image_and_both_acceptance_digests() {
    let mut target = VerifiedNativeTarget {
        client: ClientKind::ClaudeCode,
        platform: "linux".to_owned(),
        architecture: "x86_64".to_owned(),
        client_version: "1.2.3".to_owned(),
        adapter_version: "adapter-1".to_owned(),
        image_digest: "c".repeat(64),
        executable_digest: "d".repeat(64),
        native_isolation_evidence_digest: "e".repeat(64),
        native_metering_evidence_digest: "f".repeat(64),
    };
    let mut input = variant();
    assert!(target.matches(&input, "linux", "x86_64"));
    assert!(!target.matches(&input, "windows", "x86_64"));
    assert!(!target.matches(&input, "linux", "aarch64"));
    input.client_version = "1.2.4".to_owned();
    assert!(!target.matches(&input, "linux", "x86_64"));
    input = variant();
    input.worker_image_digest = "d".repeat(64);
    assert!(!target.matches(&input, "linux", "x86_64"));
    target.native_metering_evidence_digest.clear();
    assert!(!target.matches(&variant(), "linux", "x86_64"));
    assert!(target.validate().is_err());
}

#[test]
fn opencode_preserves_stored_wire_names_and_accepts_the_registry_alias() {
    assert_eq!(
        serde_json::to_string(&ClientKind::Opencode).unwrap(),
        "\"opencode\""
    );
    assert_eq!(
        serde_json::from_str::<ClientKind>("\"open-code\"").unwrap(),
        ClientKind::Opencode
    );
    assert_eq!(
        EvaluatorClient::from(ClientKind::Opencode),
        EvaluatorClient::OpenCode
    );
}
