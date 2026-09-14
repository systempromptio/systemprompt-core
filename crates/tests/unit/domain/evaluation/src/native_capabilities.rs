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

fn proof_fixture() -> (serde_json::Value, serde_json::Value, serde_json::Value) {
    let binding = serde_json::json!({"client":"claude-code","platform":"linux","architecture":"x86_64","client_version":"1.2.3","adapter_version":"adapter-1","image_config_digest":"c".repeat(64),"executable_digest":"d".repeat(64),"core_source_digest":"a".repeat(64),"astound_source_digest":"b".repeat(64)});
    let isolation = serde_json::json!({"schema_version":1,"binding":binding,"contracts":{
        "pinned_executable":true,"configuration_credentials":true,"forbidden_tools":true,"network":true,"cancellation_cleanup":true,"attempt_bound":true,"output_bound":true,"failed_evidence":true,"matched_environments":true,"judge":true,"suggestion":true},"artifacts":{"retained/native.json":"e".repeat(64)}});
    let metering = serde_json::json!({"schema_version":1,"binding":binding,"contracts":{
        "authenticated_execution_session":true,"atomic_reservation":true,"native_usage_parity":true,"failed_spend":true,"unknown_pricing":true,"attempt_output_cost_bounds":true,"stale_lease":true,"revocation":true,"idempotent_settlement":true},"artifacts":{"retained/gateway.json":"f".repeat(64)}});
    let manifest = serde_json::json!({"schema_version":1,"binding":binding,"repository_manifest_digest":null,"target":{
        "client":"claude-code","platform":"linux","architecture":"x86_64","client_version":"1.2.3","adapter_version":"adapter-1","image_digest":"c".repeat(64),"executable_digest":"d".repeat(64),"native_isolation_evidence_digest":"e".repeat(64),"native_metering_evidence_digest":"f".repeat(64)}});
    (manifest, isolation, metering)
}

fn embedded_proof(
    mut manifest: serde_json::Value,
    isolation: serde_json::Value,
    metering: serde_json::Value,
) -> systemprompt_evaluation::capabilities::proofs::EmbeddedNativeProof {
    use sha2::{Digest, Sha256};
    let isolation = isolation.to_string();
    let metering = metering.to_string();
    manifest["target"]["native_isolation_evidence_digest"] =
        hex::encode(Sha256::digest(isolation.as_bytes())).into();
    manifest["target"]["native_metering_evidence_digest"] =
        hex::encode(Sha256::digest(metering.as_bytes())).into();
    let manifest = manifest.to_string();
    let hash = hex::encode(Sha256::digest(manifest.as_bytes()));
    systemprompt_evaluation::capabilities::proofs::EmbeddedNativeProof {
        manifest: Box::leak(manifest.into_boxed_str()),
        manifest_sha256: Box::leak(hash.into_boxed_str()),
        isolation: Box::leak(isolation.into_boxed_str()),
        metering: Box::leak(metering.into_boxed_str()),
    }
}

#[test]
fn reviewed_proof_validates_content_without_registering_fixture_targets() {
    use systemprompt_evaluation::capabilities::proofs::image_config_for_target;
    let (manifest, isolation, metering) = proof_fixture();
    let proof = embedded_proof(manifest, isolation, metering);
    let accepted = proof.validate().unwrap();
    assert!(accepted.target.matches(&variant(), "linux", "x86_64"));
    assert!(!accepted.target.matches(&variant(), "windows", "x86_64"));
    assert!(
        image_config_for_target(&accepted.target, &format!("sha256:{}", "c".repeat(64))).is_err()
    );
    assert!(
        admit_variant(&variant()).is_err(),
        "Validating fixture evidence cannot register production targets"
    );
    let mut corrupted = proof;
    corrupted.manifest_sha256 = "0000000000000000000000000000000000000000000000000000000000000000";
    assert!(corrupted.validate().is_err());
    corrupted = proof;
    corrupted.isolation = "{}";
    assert!(corrupted.validate().is_err());
}

#[test]
fn reviewed_proofs_reject_missing_failed_or_mismatched_contracts_even_when_rehashed() {
    for mutation in 0..8 {
        let (mut manifest, mut isolation, mut metering) = proof_fixture();
        match mutation {
            0 => {
                isolation["contracts"]
                    .as_object_mut()
                    .unwrap()
                    .remove("network");
            },
            1 => metering["contracts"]["failed_spend"] = false.into(),
            2 => isolation["binding"]["architecture"] = "aarch64".into(),
            3 => manifest["binding"]["adapter_version"] = "different".into(),
            4 => manifest["repository_manifest_digest"] = "d".repeat(64).into(),
            5 => metering["artifacts"] = serde_json::json!({}),
            6 => metering["schema_version"] = 2.into(),
            _ => isolation["artifacts"] = serde_json::json!({"../unretained":"a".repeat(64)}),
        }
        assert!(
            embedded_proof(manifest, isolation, metering)
                .validate()
                .is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn immutable_image_config_ids_and_repository_manifests_are_distinct() {
    use systemprompt_evaluation::capabilities::proofs::ImmutableImage;
    let hash = "c".repeat(64);
    let local = format!("sha256:{hash}");
    let remote = format!("registry.example/evaluator@sha256:{hash}");
    assert_eq!(
        ImmutableImage::parse(&local).unwrap(),
        ImmutableImage::LocalConfig(&hash)
    );
    assert_eq!(
        ImmutableImage::parse(&remote).unwrap(),
        ImmutableImage::RepositoryManifest(&hash)
    );
    for value in [
        "latest",
        "sha256:abc",
        "@sha256:abc",
        "xsha256:abc",
        "-option@sha256:abc",
    ] {
        assert!(ImmutableImage::parse(value).is_err());
    }
    let (mut manifest, isolation, metering) = proof_fixture();
    manifest["repository_manifest_digest"] = "b".repeat(64).into();
    manifest["target"]["image_digest"] = "b".repeat(64).into();
    let accepted = embedded_proof(manifest, isolation, metering)
        .validate()
        .unwrap();
    assert_ne!(
        accepted.target.image_digest,
        accepted.binding.image_config_digest
    );
}
