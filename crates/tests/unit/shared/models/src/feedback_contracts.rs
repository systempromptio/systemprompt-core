use chrono::Utc;
use systemprompt_identifiers::{
    ConsumerInstallationId, DependencyVerificationId, ManagedResourceId, ManagedSourceId,
    PublicationId, ResourceRevisionId,
};
use systemprompt_models::feedback::receipts::{
    ConsumerReceiptRequest, FileReadback, ReadbackStatus,
};
use systemprompt_models::feedback::verification::{
    DependencyVerificationInput, DependencyVerificationManifest, DependencyVerificationRequest,
    VerifiedRevisionManifest,
};
use systemprompt_models::feedback::{ContentDigest, EvaluatorClient, FeedbackContractError};

fn node(id: &str, dependencies: &[&str]) -> DependencyVerificationInput {
    DependencyVerificationInput {
        revision_id: ResourceRevisionId::new(id),
        source_id: ManagedSourceId::new("registered-source"),
        exact_commit: "a".repeat(40),
        relative_root: "skills/example".to_owned(),
        dependencies: dependencies
            .iter()
            .map(|id| ResourceRevisionId::new(*id))
            .collect(),
    }
}

#[test]
fn complete_dependency_graph_rejects_missing_cycles_and_unreachable_nodes() {
    let mut request = DependencyVerificationRequest {
        root_revision_id: ResourceRevisionId::new("root"),
        revisions: vec![node("root", &["child"]), node("child", &[])],
    };
    assert!(request.validate().is_ok());
    request.revisions.pop();
    assert_eq!(
        request.validate(),
        Err(FeedbackContractError::IncompleteManifest)
    );
    request.revisions.push(node("child", &["root"]));
    assert_eq!(
        request.validate(),
        Err(FeedbackContractError::DependencyCycle)
    );
    request.revisions[1] = node("child", &[]);
    request.revisions.push(node("unrelated", &[]));
    assert_eq!(
        request.validate(),
        Err(FeedbackContractError::IncompleteManifest)
    );
}

#[test]
fn dependency_provenance_rejects_relative_escape_commit_and_duplicate_edges() {
    let mut request = DependencyVerificationRequest {
        root_revision_id: ResourceRevisionId::new("root"),
        revisions: vec![node("root", &[])],
    };
    request.revisions[0].relative_root = "../secret".to_owned();
    assert_eq!(request.validate(), Err(FeedbackContractError::InvalidPath));
    request.revisions[0] = node("root", &[]);
    request.revisions[0].exact_commit = "main".to_owned();
    assert_eq!(
        request.validate(),
        Err(FeedbackContractError::IncompleteManifest)
    );
    request.revisions = vec![node("root", &["child", "child"]), node("child", &[])];
    assert_eq!(
        request.validate(),
        Err(FeedbackContractError::IncompleteManifest)
    );
}

#[test]
fn manifest_requires_byte_and_mode_evidence_for_every_revision() {
    let mut manifest = DependencyVerificationManifest {
        id: DependencyVerificationId::generate(),
        version: 1,
        root_revision_id: ResourceRevisionId::new("root"),
        bundle_digest: ContentDigest::of(b"bundle"),
        revisions: vec![VerifiedRevisionManifest {
            provenance: node("root", &[]),
            content_digest: ContentDigest::of(b"content"),
            file_count: 1,
            bytes_verified: true,
            modes_verified: true,
        }],
        verified_at: Utc::now(),
    };
    assert!(manifest.validate_complete().is_ok());
    manifest.revisions[0].modes_verified = false;
    assert_eq!(
        manifest.validate_complete(),
        Err(FeedbackContractError::IncompleteManifest)
    );
}

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
fn client_alias_preserves_registry_serialization() {
    assert_eq!(
        serde_json::to_string(&EvaluatorClient::OpenCode).unwrap(),
        "\"open-code\""
    );
    assert_eq!(
        serde_json::from_str::<EvaluatorClient>("\"opencode\"").unwrap(),
        EvaluatorClient::OpenCode
    );
    assert!(serde_json::from_str::<ContentDigest>("\"not-a-digest\"").is_err());
}

#[test]
fn unknown_resource_attribution_preserves_authenticated_identity() {
    use systemprompt_identifiers::{DeviceId, NativeSessionId, ResourceInvocationId, UserId};
    use systemprompt_models::feedback::analytics::{
        InvocationConsumerIdentity, InvocationResourceAttribution, NormalizedInvocationFact,
    };
    let mut fact = NormalizedInvocationFact {
        invocation_id: ResourceInvocationId::new("invocation"),
        occurred_at: Utc::now(),
        consumer: InvocationConsumerIdentity::Authenticated {
            consumer_id: UserId::new("consumer"),
            device_id: DeviceId::try_new("device").expect("nonempty fixture device"),
            host: EvaluatorClient::Codex,
            session_id: NativeSessionId::new("native-session"),
        },
        attribution: InvocationResourceAttribution::Unknown,
        skill: None,
        succeeded: false,
        latency_micros: None,
    };
    let identity = fact.consumer.clone();
    fact.attribution = InvocationResourceAttribution::Verified {
        resource_id: ManagedResourceId::new("resource"),
        revision_id: ResourceRevisionId::new("revision"),
    };
    assert_eq!(identity, fact.consumer);
}

#[test]
fn normalized_changes_reject_kind_mismatch_and_retain_unknown_failed_spend() {
    use systemprompt_identifiers::{AnalyticsChangeId, AnalyticsFactId};
    use systemprompt_models::feedback::analytics::{
        AnalyticsChange, AnalyticsChangeOperation, AnalyticsFactKey, AnalyticsFactKind,
        InvocationConsumerIdentity, NormalizedAnalyticsFact, NormalizedRequestFact, RecordedSpend,
    };
    let key = AnalyticsFactKey {
        kind: AnalyticsFactKind::Request,
        source: "gateway".to_owned(),
        id: AnalyticsFactId::new("request"),
    };
    let mut change = AnalyticsChange {
        change_id: AnalyticsChangeId::generate(),
        key: key.clone(),
        revision: 1,
        occurred_at: Utc::now(),
        recorded_at: Utc::now(),
        operation: AnalyticsChangeOperation::Replace {
            fact: NormalizedAnalyticsFact::Request(NormalizedRequestFact {
                request_key: key,
                occurred_at: Utc::now(),
                consumer: InvocationConsumerIdentity::HistoricalUnknown,
                succeeded: false,
                spend: RecordedSpend::UnknownPricing,
                input_tokens: None,
                output_tokens: None,
                latency_micros: None,
            }),
        },
    };
    assert!(change.validate().is_ok());
    let encoded = serde_json::to_string(&change).unwrap();
    assert!(encoded.contains("unknown_pricing"));
    assert!(encoded.contains("historical_unknown"));
    change.key.kind = AnalyticsFactKind::Invocation;
    assert_eq!(
        change.validate(),
        Err(FeedbackContractError::IncompleteManifest)
    );
}

#[test]
fn installation_host_aliases_preserve_existing_codex_and_opencode_names() {
    assert!(EvaluatorClient::Codex.accepts_host_name("codex-cli"));
    assert!(EvaluatorClient::Codex.accepts_host_name("codex"));
    assert!(EvaluatorClient::OpenCode.accepts_host_name("opencode"));
    assert!(EvaluatorClient::OpenCode.accepts_host_name("open-code"));
    assert!(!EvaluatorClient::Codex.accepts_host_name("hermes"));
}

#[test]
fn invocation_skill_identity_is_optional_on_the_wire_and_round_trips() {
    use systemprompt_identifiers::{MarketplaceId, PluginId, ResourceInvocationId};
    use systemprompt_models::feedback::analytics::{
        InvocationConsumerIdentity, InvocationResourceAttribution, InvocationSkillIdentity,
        NormalizedInvocationFact,
    };
    let legacy = serde_json::json!({
        "invocation_id": "invocation",
        "occurred_at": "2026-09-16T00:00:00Z",
        "consumer": {"status": "historical_unknown"},
        "attribution": {"status": "unknown"},
        "succeeded": true,
        "latency_micros": null
    });
    let fact: NormalizedInvocationFact = serde_json::from_value(legacy)
        .expect("a fact written before the skill identity still reads");
    assert!(fact.skill.is_none());

    let fact = NormalizedInvocationFact {
        invocation_id: ResourceInvocationId::new("invocation"),
        occurred_at: Utc::now(),
        consumer: InvocationConsumerIdentity::HistoricalUnknown,
        attribution: InvocationResourceAttribution::Unknown,
        skill: Some(InvocationSkillIdentity {
            plugin_id: PluginId::new("astound-india-ba"),
            skill: "astound-india-ba:ba-bug-logging".to_owned(),
            marketplace_id: Some(MarketplaceId::new("astound-india-dev")),
            source: Some("bundle:india".to_owned()),
            source_hash: Some("abc".to_owned()),
            marketplace_hash: Some("def".to_owned()),
        }),
        succeeded: true,
        latency_micros: None,
    };
    let json = serde_json::to_value(&fact).expect("serialises");
    assert_eq!(json["skill"]["source"], "bundle:india");
    let back: NormalizedInvocationFact = serde_json::from_value(json).expect("round-trips");
    assert_eq!(back, fact);
}
