use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use systemprompt_bridge::feedback::credentials::Enrollment;
use systemprompt_bridge::feedback::outbox::{Delivery, Outbox, OutboxScope};
use systemprompt_bridge::feedback_capture::capture_host;
use systemprompt_bridge::gateway::GatewayClient;
use systemprompt_bridge::gateway::manifest::{MANIFEST_SCHEMA_VERSION, SignedManifest, SkillEntry};
use systemprompt_bridge::gateway::manifest_version::ManifestVersion;
use systemprompt_bridge::host_sync::{HostSyncCtx, HostWarnings};
use systemprompt_bridge::ids::{BearerToken, LoopbackSecret, Sha256Digest, SkillId, SkillName};
use systemprompt_bridge::proxy::LoopbackEndpoint;
use systemprompt_identifiers::{
    DeviceId, ManagedResourceId, PublicationId, ResourceRevisionId, UserId, ValidatedUrl,
};
use systemprompt_models::bridge::manifest::SkillPublication;
use systemprompt_models::feedback::receipts::{
    ConsumerInstallationPlan, ConsumerReceiptResponse, FileReadback, InstallationPlanFile,
    ReadbackStatus, ReceiptAcknowledgement,
};
use systemprompt_models::feedback::{ContentDigest, EvaluatorClient};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

static WARNINGS: HostWarnings = HostWarnings::new();
static POLICY: std::sync::LazyLock<systemprompt_bridge::config::store::PolicyStore> =
    std::sync::LazyLock::new(|| {
        systemprompt_bridge::config::store::PolicyStore::new(
            systemprompt_bridge::config::store::managed_policy_store(),
        )
    });
static REGISTRY: std::sync::LazyLock<systemprompt_bridge::mcp_registry::McpRegistry> =
    std::sync::LazyLock::new(std::collections::HashMap::new);
static START_MENU: std::sync::LazyLock<systemprompt_bridge::probe_cache::StartMenuCache> =
    std::sync::LazyLock::new(systemprompt_bridge::probe_cache::StartMenuCache::default);
static LOOPBACK: std::sync::LazyLock<LoopbackEndpoint> = std::sync::LazyLock::new(|| {
    LoopbackEndpoint::new(
        systemprompt_bridge::proxy::DEFAULT_PROXY_PORT,
        Some(LoopbackSecret::new("capture-loopback-secret")),
    )
});

fn publication() -> SkillPublication {
    SkillPublication {
        publication_id: PublicationId::new("publication"),
        resource_id: ManagedResourceId::new("resource"),
        revision_id: ResourceRevisionId::new("revision"),
        generation: 1,
        bundle_digest: Sha256Digest::try_new(ContentDigest::of(b"bundle").as_str()).unwrap(),
    }
}

fn configure_cowork_session(root: &Path) -> std::path::PathBuf {
    let session = root.join("cowork-session/org");
    fs::create_dir_all(session.join("cowork_plugins")).unwrap();
    let config_dir = root.join("config/systemprompt");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(
        config_dir.join("systemprompt-bridge.toml"),
        format!(
            "gateway_url = 'http://gw.invalid:7000'\n\n[cowork]\nsession_org_dir = '{}'\n",
            session.display()
        ),
    )
    .unwrap();
    session
}

fn cowork_manifest(publication: SkillPublication) -> SignedManifest {
    let mut manifest = manifest_for("claude-desktop", publication);
    let plugin = plugin_entry_for_capture();
    manifest.skills[0].plugins = vec![plugin.id.clone()];
    manifest.plugins = vec![plugin];
    manifest
}

#[test]
fn cowork_capture_keeps_failed_generation_pending_when_its_required_plugin_root_disappears() {
    with_capture_sandbox(|root| {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let _session = configure_cowork_session(root);
                let skill_root = root.join("capture-plugin/skills/capture-skill");
                fs::create_dir_all(&skill_root).unwrap();
                fs::write(skill_root.join("SKILL.md"), "# installed by Cowork\n").unwrap();
                fs::write(skill_root.join("operator-note.txt"), "keep me\n").unwrap();

                let server = MockServer::start().await;
                let feedback_root = systemprompt_bridge::feedback::metadata_root().unwrap();
                fs::create_dir_all(&feedback_root).unwrap();
                let enrollment = Enrollment::new(
                    &server.uri(),
                    DeviceId::try_new("cowork-capture-device").unwrap(),
                    UserId::new("consumer"),
                    BearerToken::new("sp_device_capture"),
                )
                .unwrap();
                enrollment.save(&feedback_root).unwrap();
                let outbox = Outbox::new(
                    enrollment.outbox_path(&feedback_root),
                    OutboxScope::from_enrollment(&enrollment),
                );
                let publication = publication();
                mount_plan_and_receipt(
                    &server,
                    &publication,
                    EvaluatorClient::ClaudeDesktop,
                    true,
                    None,
                )
                .await;
                let manifest = cowork_manifest(publication);
                let bearer = BearerToken::default();
                let ctx = context(&manifest, &server.uri(), &bearer, root);

                assert!(capture_host("claude-desktop", &ctx).await.is_err());
                assert_eq!(outbox.pending_installations().unwrap().len(), 1);
                assert!(outbox.entries().unwrap().is_empty());
                assert_eq!(
                    fs::read_to_string(skill_root.join("operator-note.txt")).unwrap(),
                    "keep me\n"
                );

                server.reset().await;
                fs::remove_file(skill_root.join("SKILL.md")).unwrap();
                assert!(capture_host("claude-desktop", &ctx).await.is_err());
                let pending = outbox.pending_installations().unwrap();
                assert_eq!(
                    pending.len(),
                    1,
                    "the reserved failed generation remains retryable"
                );
                assert!(!pending[0].1.superseded);
                assert!(outbox.entries().unwrap().is_empty());
                assert!(server.received_requests().await.unwrap().is_empty());
                assert_eq!(
                    fs::read_to_string(skill_root.join("operator-note.txt")).unwrap(),
                    "keep me\n",
                    "capture never owns unrelated plugin files"
                );
            });
    });
}

fn manifest_for(host: &str, publication: SkillPublication) -> SignedManifest {
    let mut manifest = manifest();
    manifest.enabled_hosts = vec![host.to_owned()];
    manifest.skills[0].hosts = vec![host.to_owned()];
    manifest.skills[0].publication = Some(publication);
    manifest
}

fn plan_for(host: EvaluatorClient, publication: &SkillPublication) -> ConsumerInstallationPlan {
    let mut plan = plan();
    plan.host = host;
    plan.publication_id = publication.publication_id.clone();
    plan.resource_id = publication.resource_id.clone();
    plan.revision_id = publication.revision_id.clone();
    plan.generation = publication.generation;
    plan.bundle_digest =
        ContentDigest::try_from(publication.bundle_digest.as_str().to_owned()).unwrap();
    for canonical in &mut plan.canonical_files {
        canonical.revision_id = publication.revision_id.clone();
    }
    plan.runtime_files[0].path = format!(
        ".systemprompt-source/{}/scripts/run.sh",
        publication.revision_id
    );
    plan
}

fn native_skill_root(root: &Path, host: &str) -> std::path::PathBuf {
    match host {
        "opencode" => root.join("config/opencode/skills/capture-skill"),
        "hermes" => root.join(".hermes/skills/capture_skill"),
        other => panic!("unsupported fixture host {other}"),
    }
}

#[test]
fn native_host_readback_failure_keeps_foreign_files_and_retry_records_only_verified_bytes() {
    for (host_name, host_kind) in [
        ("opencode", EvaluatorClient::OpenCode),
        ("hermes", EvaluatorClient::Hermes),
    ] {
        with_capture_sandbox(|root| {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    let skill_root = native_skill_root(root, host_name);
                    fs::create_dir_all(&skill_root).unwrap();
                    fs::write(skill_root.join("SKILL.md"), "# old\n").unwrap();
                    fs::write(skill_root.join("foreign.txt"), "operator-owned\n").unwrap();
                    let server = MockServer::start().await;
                    let feedback_root = systemprompt_bridge::feedback::metadata_root().unwrap();
                    fs::create_dir_all(&feedback_root).unwrap();
                    let enrollment = Enrollment::new(
                        &server.uri(),
                        DeviceId::try_new(format!("{host_name}-device")).unwrap(),
                        UserId::new("consumer"),
                        BearerToken::new("sp_device_capture"),
                    )
                    .unwrap();
                    enrollment.save(&feedback_root).unwrap();
                    let outbox = Outbox::new(
                        enrollment.outbox_path(&feedback_root),
                        OutboxScope::from_enrollment(&enrollment),
                    );

                    let first_publication = publication();
                    let mut mismatched = plan_for(host_kind, &first_publication);
                    mismatched.canonical_files[0].digest = ContentDigest::of(b"wrong digest");
                    Mock::given(method("GET"))
                        .and(path("/api/v1/consumer/resources/resource/publications/publication/bundle"))
                        .respond_with(ResponseTemplate::new(200).set_body_json(mismatched))
                        .expect(1)
                        .mount(&server)
                        .await;
                    let first_manifest = manifest_for(host_name, first_publication);
                    let bearer = BearerToken::default();
                    let first_ctx = context(&first_manifest, &server.uri(), &bearer, root);
                    assert!(
                        capture_host(host_name, &first_ctx).await.is_err(),
                        "canonical digest mismatch must not produce a receipt for {host_name}"
                    );
                    assert!(outbox.entries().unwrap().is_empty());
                    assert_eq!(outbox.pending_installations().unwrap().len(), 1);
                    assert_eq!(
                        fs::read_to_string(skill_root.join("foreign.txt")).unwrap(),
                        "operator-owned\n"
                    );

                    server.reset().await;
                    let mut repaired_publication = publication();
                    repaired_publication.publication_id = PublicationId::new("publication-retry");
                    repaired_publication.revision_id = ResourceRevisionId::new("revision-retry");
                    repaired_publication.generation = 2;
                    repaired_publication.bundle_digest =
                        Sha256Digest::try_new(ContentDigest::of(b"bundle retry").as_str()).unwrap();
                    let repaired_plan = plan_for(host_kind, &repaired_publication);
                    Mock::given(method("GET"))
                        .and(path("/api/v1/consumer/resources/resource/publications/publication-retry/bundle"))
                        .respond_with(ResponseTemplate::new(200).set_body_json(repaired_plan))
                        .expect(1)
                        .mount(&server)
                        .await;
                    let response = ConsumerReceiptResponse {
                        receipt_id: systemprompt_identifiers::InstallationReceiptId::new(
                            format!("{host_name}-receipt"),
                        ),
                        acknowledgement: ReceiptAcknowledgement::Accepted,
                        acknowledged_at: chrono::Utc::now(),
                        fully_verified: true,
                    };
                    Mock::given(method("POST"))
                        .and(path("/api/v1/consumer/receipts"))
                        .respond_with(ResponseTemplate::new(200).set_body_json(response))
                        .expect(1)
                        .mount(&server)
                        .await;
                    let repaired_manifest = manifest_for(host_name, repaired_publication);
                    let repaired_ctx = context(&repaired_manifest, &server.uri(), &bearer, root);
                    let outcome = capture_host(host_name, &repaired_ctx).await.unwrap();
                    assert_eq!(outcome.recovered, 1);
                    assert!(!outcome.undelivered);
                    assert_eq!(
                        fs::read_to_string(skill_root.join("foreign.txt")).unwrap(),
                        "operator-owned\n",
                        "runtime reconciliation never claims a foreign file"
                    );
                    let pending = outbox.pending_installations().unwrap();
                    assert_eq!(pending.len(), 1, "failed generation remains as evidence");
                    assert!(pending[0].1.superseded);
                    let entries = outbox.entries().unwrap();
                    assert_eq!(entries.len(), 1);
                    assert!(matches!(entries[0].1.delivery, Delivery::Acknowledged(_)));
                    assert_eq!(entries[0].1.request.host, host_kind);
                    assert!(entries[0].1.request.runtime_files.iter().all(|file| {
                        file.content_check == ReadbackStatus::Verified
                            && file.digest == ContentDigest::of(&fs::read(skill_root.join(&file.path)).unwrap())
                    }));
                });
        });
    }
}


fn plugin_entry_for_capture() -> systemprompt_bridge::gateway::manifest::PluginEntry {
    systemprompt_bridge::gateway::manifest::PluginEntry {
        id: systemprompt_bridge::ids::PluginId::try_new("capture-plugin").unwrap(),
        version: "1.0.0".into(),
        sha256: Sha256Digest::try_new("0".repeat(64)).unwrap(),
        files: vec![],
        hooks: systemprompt_models::services::PluginHooksRef::default(),
    }
}

fn claude_manifest(publication: SkillPublication) -> SignedManifest {
    let mut manifest = manifest_for("claude-code", publication);
    let plugin = plugin_entry_for_capture();
    manifest.skills[0].plugins = vec![plugin.id.clone()];
    manifest.plugins = vec![plugin.clone()];
    manifest.marketplaces = vec![
        systemprompt_bridge::gateway::manifest::ManifestMarketplace {
            id: systemprompt_identifiers::MarketplaceId::new("capture-marketplace"),
            name: "Capture marketplace".into(),
            plugin_ids: vec![plugin.id],
            allow_cross_marketplace_dependencies_on: vec![],
            external_marketplaces: vec![],
        },
    ];
    manifest
}

fn claude_skill_root(root: &Path) -> std::path::PathBuf {
    root.join(
        ".claude/plugins/cache/capture-marketplace/capture-plugin/current/skills/capture-skill",
    )
}

async fn mount_plan_and_receipt(
    server: &MockServer,
    publication: &SkillPublication,
    host: EvaluatorClient,
    mismatch: bool,
    receipt: Option<&str>,
) {
    let mut plan = plan_for(host, publication);
    if mismatch {
        plan.canonical_files[0].digest = ContentDigest::of(b"bytes the gateway did not install");
    }
    Mock::given(method("GET"))
        .and(path(format!(
            "/api/v1/consumer/resources/resource/publications/{}/bundle",
            publication.publication_id
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(plan))
        .expect(1)
        .mount(server)
        .await;
    if let Some(receipt_id) = receipt {
        Mock::given(method("POST"))
            .and(path("/api/v1/consumer/receipts"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(ConsumerReceiptResponse {
                    receipt_id: systemprompt_identifiers::InstallationReceiptId::new(receipt_id),
                    acknowledgement: ReceiptAcknowledgement::Accepted,
                    acknowledged_at: chrono::Utc::now(),
                    fully_verified: true,
                }),
            )
            .expect(1)
            .mount(server)
            .await;
    }
}

fn repaired_publication() -> SkillPublication {
    let mut publication = publication();
    publication.publication_id = PublicationId::new("publication-repaired");
    publication.revision_id = ResourceRevisionId::new("revision-repaired");
    publication.generation = 2;
    publication.bundle_digest =
        Sha256Digest::try_new(ContentDigest::of(b"repaired bundle").as_str()).unwrap();
    publication
}

#[test]
fn claude_native_cache_rejects_mismatched_evidence_then_acknowledges_repaired_generation() {
    with_capture_sandbox(|root| {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let skill_root = claude_skill_root(root);
                fs::create_dir_all(&skill_root).unwrap();
                fs::write(skill_root.join("SKILL.md"), "# installed by Claude\n").unwrap();
                fs::write(skill_root.join("operator-note.txt"), "keep me\n").unwrap();
                let server = MockServer::start().await;
                let feedback_root = systemprompt_bridge::feedback::metadata_root().unwrap();
                fs::create_dir_all(&feedback_root).unwrap();
                let enrollment = Enrollment::new(
                    &server.uri(),
                    DeviceId::try_new("claude-capture-device").unwrap(),
                    UserId::new("consumer"),
                    BearerToken::new("sp_device_capture"),
                )
                .unwrap();
                enrollment.save(&feedback_root).unwrap();
                let outbox = Outbox::new(
                    enrollment.outbox_path(&feedback_root),
                    OutboxScope::from_enrollment(&enrollment),
                );
                let first = publication();
                mount_plan_and_receipt(&server, &first, EvaluatorClient::ClaudeCode, true, None)
                    .await;
                let first_manifest = claude_manifest(first);
                let bearer = BearerToken::default();
                let first_ctx = context(&first_manifest, &server.uri(), &bearer, root);
                assert!(capture_host("claude-code", &first_ctx).await.is_err());
                assert!(
                    outbox.entries().unwrap().is_empty(),
                    "wrong bytes never become evidence"
                );
                assert_eq!(outbox.pending_installations().unwrap().len(), 1);
                assert_eq!(
                    fs::read_to_string(skill_root.join("operator-note.txt")).unwrap(),
                    "keep me\n"
                );

                server.reset().await;
                let repaired = repaired_publication();
                mount_plan_and_receipt(
                    &server,
                    &repaired,
                    EvaluatorClient::ClaudeCode,
                    false,
                    Some("claude-repaired-receipt"),
                )
                .await;
                let repaired_manifest = claude_manifest(repaired);
                let repaired_ctx = context(&repaired_manifest, &server.uri(), &bearer, root);
                let outcome = capture_host("claude-code", &repaired_ctx).await.unwrap();
                assert_eq!(outcome.recovered, 1);
                assert!(!outcome.undelivered);
                assert_eq!(
                    fs::read_to_string(skill_root.join("operator-note.txt")).unwrap(),
                    "keep me\n",
                    "runtime reconciliation preserves unowned native files"
                );
                let pending = outbox.pending_installations().unwrap();
                assert_eq!(pending.len(), 1);
                assert!(
                    pending[0].1.superseded,
                    "failed generation remains but is superseded"
                );
                let entries = outbox.entries().unwrap();
                assert_eq!(entries.len(), 1);
                assert!(matches!(entries[0].1.delivery, Delivery::Acknowledged(_)));
                assert_eq!(entries[0].1.request.host, EvaluatorClient::ClaudeCode);
                assert!(entries[0].1.request.runtime_files.iter().all(|file| {
                    file.content_check == ReadbackStatus::Verified
                        && file.mode_check == ReadbackStatus::Verified
                }));
            });
    });
}

#[test]
fn codex_emitter_roots_require_both_source_and_versioned_cache_before_capture() {
    use systemprompt_bridge::host_sync::HostSync as _;
    use systemprompt_bridge::integration::codex_cli::CodexCliSync;

    with_capture_sandbox(|root| {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let publication = publication();
                let manifest = manifest_for("codex-cli", publication);
                let bearer = BearerToken::default();
                let ctx = context(&manifest, "http://127.0.0.1:9", &bearer, root);
                CodexCliSync.apply(&ctx).await.expect("real Codex host emitter applies");
                let source = root.join(
                    ".codex/.systemprompt/marketplace/plugins/systemprompt-managed/skills/capture_skill",
                );
                assert!(source.join("SKILL.md").is_file());
                let cache_base = root.join(".codex/plugins/cache/systemprompt/systemprompt-managed");
                let cache_version = fs::read_dir(&cache_base)
                    .unwrap()
                    .find_map(|entry| {
                        let path = entry.unwrap().path();
                        path.is_dir().then_some(path)
                    })
                    .expect("Codex emitter creates a versioned cache");
                let cache_skill = cache_version.join("skills/capture_skill");
                assert!(cache_skill.join("SKILL.md").is_file());
                fs::remove_file(cache_skill.join("SKILL.md")).unwrap();

                let server = MockServer::start().await;
                let feedback_root = systemprompt_bridge::feedback::metadata_root().unwrap();
                fs::create_dir_all(&feedback_root).unwrap();
                let enrollment = Enrollment::new(
                    &server.uri(),
                    DeviceId::try_new("codex-capture-device").unwrap(),
                    UserId::new("consumer"),
                    BearerToken::new("sp_device_capture"),
                )
                .unwrap();
                enrollment.save(&feedback_root).unwrap();
                let capture_ctx = context(&manifest, &server.uri(), &bearer, root);
                assert!(
                    capture_host("codex-cli", &capture_ctx).await.is_err(),
                    "one surviving copy cannot stand in for the missing Codex cache copy"
                );
                assert!(server.received_requests().await.unwrap().is_empty());
                let outbox = Outbox::new(
                    enrollment.outbox_path(&feedback_root),
                    OutboxScope::from_enrollment(&enrollment),
                );
                assert!(outbox.pending_installations().unwrap().is_empty());
                assert!(outbox.entries().unwrap().is_empty());
                assert!(source.join("SKILL.md").is_file(), "source copy remains intact");
            });
    });
}

fn manifest() -> SignedManifest {
    SignedManifest {
        min_schema_version: MANIFEST_SCHEMA_VERSION,
        min_bridge_version: None,
        manifest_version: ManifestVersion::try_new("2026-04-30T12:00:00Z-deadbeef").unwrap(),
        issued_at: chrono::Utc::now(),
        not_before: chrono::Utc::now(),
        user_id: UserId::new("consumer"),
        tenant_id: None,
        user: None,
        plugins: vec![],
        skills: vec![SkillEntry {
            publication: Some(publication()),
            id: SkillId::try_new("capture_skill").unwrap(),
            name: SkillName::try_new("capture").unwrap(),
            description: "capture".into(),
            file_path: "capture/SKILL.md".into(),
            tags: vec![],
            sha256: Sha256Digest::try_new("0".repeat(64)).unwrap(),
            instructions: "capture".into(),
            hosts: vec!["opencode".into()],
            plugins: vec![],
        }],
        rules: vec![],
        agents: vec![],
        hooks: vec![],
        managed_mcp_servers: vec![],
        revocations: vec![],
        enabled_hosts: vec!["opencode".into()],
        host_model_protocols: Default::default(),
        artifacts: vec![],
        allow_claude_ai_connectors: false,
        auto_update: Default::default(),
        diagnostics: vec![],
        marketplaces: vec![],
    }
}

fn plan() -> ConsumerInstallationPlan {
    let content = b"echo safe".to_vec();
    ConsumerInstallationPlan {
        publication_id: PublicationId::new("publication"),
        resource_id: ManagedResourceId::new("resource"),
        revision_id: ResourceRevisionId::new("revision"),
        generation: 1,
        bundle_digest: ContentDigest::of(b"bundle"),
        host: EvaluatorClient::OpenCode,
        canonical_files: vec![FileReadback {
            revision_id: ResourceRevisionId::new("revision"),
            path: "scripts/run.sh".into(),
            digest: ContentDigest::of(&content),
            bytes: content.len() as u64,
            executable: true,
            content_check: ReadbackStatus::Unavailable,
            mode_check: ReadbackStatus::Unavailable,
        }],
        runtime_files: vec![
            InstallationPlanFile {
                path: ".systemprompt-source/revision/scripts/run.sh".into(),
                bytes: content.clone(),
                executable: true,
            },
            InstallationPlanFile {
                path: "scripts/run.sh".into(),
                bytes: content,
                executable: true,
            },
            InstallationPlanFile {
                path: "SKILL.md".into(),
                bytes: b"# Captured\n".to_vec(),
                executable: false,
            },
        ],
    }
}

fn context<'a>(
    manifest: &'a SignedManifest,
    gateway: &str,
    bearer: &'a BearerToken,
    root: &'a Path,
) -> HostSyncCtx<'a> {
    let client = Box::leak(Box::new(GatewayClient::new(
        ValidatedUrl::try_new(gateway).unwrap(),
        reqwest::Client::new(),
    )));
    let mappings = Box::leak(Box::new(BTreeMap::new()));
    HostSyncCtx {
        policy_store: &POLICY,
        warnings: &WARNINGS,
        manifest,
        org_plugins_root: root,
        plugin_mcp_servers: mappings,
        client,
        bearer,
        loopback: &LOOPBACK,
        mcp_registry: &REGISTRY,
        start_menu: &START_MENU,
    }
}

fn with_capture_sandbox(body: impl FnOnce(&Path)) {
    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join("config");
    let skills = config.join("opencode/skills/capture-skill");
    fs::create_dir_all(&skills).unwrap();
    fs::write(skills.join("SKILL.md"), "# old\n").unwrap();
    let root = temp.path().display().to_string();
    temp_env::with_vars(
        [
            ("HOME", Some(root.clone())),
            ("XDG_CONFIG_HOME", Some(config.display().to_string())),
            ("XDG_STATE_HOME", Some(root)),
        ],
        || body(temp.path()),
    );
}

#[test]
fn capture_host_materializes_receipt_and_acknowledges_the_durable_outbox() {
    with_capture_sandbox(|root| {
        tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
            let server = MockServer::start().await;
            Mock::given(method("GET")).and(path("/api/v1/consumer/resources/resource/publications/publication/bundle"))
                .and(header("authorization", "Bearer sp_device_capture")).respond_with(ResponseTemplate::new(200).set_body_json(plan())).mount(&server).await;
            let receipt = ConsumerReceiptResponse { receipt_id: systemprompt_identifiers::InstallationReceiptId::new("receipt"), acknowledgement: ReceiptAcknowledgement::Accepted, acknowledged_at: chrono::Utc::now(), fully_verified: true };
            Mock::given(method("POST")).and(path("/api/v1/consumer/receipts")).and(header("authorization", "Bearer sp_device_capture"))
                .respond_with(ResponseTemplate::new(200).set_body_json(receipt)).expect(1).mount(&server).await;
            let feedback_root = systemprompt_bridge::feedback::metadata_root().unwrap(); fs::create_dir_all(&feedback_root).unwrap();
            let enrollment = Enrollment::new(&server.uri(), DeviceId::try_new("capture-device").unwrap(), UserId::new("consumer"), BearerToken::new("sp_device_capture")).unwrap(); enrollment.save(&feedback_root).unwrap();
            let m = manifest(); let bearer = BearerToken::default(); let ctx = context(&m, &server.uri(), &bearer, root);
            let outcome = capture_host("opencode", &ctx).await.unwrap();
            assert_eq!(outcome.recovered, 1); assert_eq!(outcome.remaining, 0); assert!(!outcome.undelivered);
            let outbox = Outbox::new(enrollment.outbox_path(&feedback_root), OutboxScope::from_enrollment(&enrollment));
            assert!(outbox.pending_installations().unwrap().is_empty());
            assert!(matches!(outbox.entries().unwrap()[0].1.delivery, Delivery::Acknowledged(_)));
            assert!(root.join("config/opencode/skills/capture-skill/.systemprompt-source/revision/scripts/run.sh").is_file());
        });
    });
}

#[test]
fn capture_host_keeps_the_reserved_plan_when_the_gateway_cannot_supply_a_bundle() {
    with_capture_sandbox(|root| {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let server = MockServer::start().await;
                Mock::given(method("GET"))
                    .and(path(
                        "/api/v1/consumer/resources/resource/publications/publication/bundle",
                    ))
                    .respond_with(ResponseTemplate::new(503))
                    .expect(1)
                    .mount(&server)
                    .await;
                let feedback_root = systemprompt_bridge::feedback::metadata_root().unwrap();
                fs::create_dir_all(&feedback_root).unwrap();
                let enrollment = Enrollment::new(
                    &server.uri(),
                    DeviceId::try_new("capture-device").unwrap(),
                    UserId::new("consumer"),
                    BearerToken::new("sp_device_capture"),
                )
                .unwrap();
                enrollment.save(&feedback_root).unwrap();
                let m = manifest();
                let bearer = BearerToken::default();
                let ctx = context(&m, &server.uri(), &bearer, root);
                assert!(capture_host("opencode", &ctx).await.is_err());
                let outbox = Outbox::new(
                    enrollment.outbox_path(&feedback_root),
                    OutboxScope::from_enrollment(&enrollment),
                );
                assert_eq!(outbox.pending_installations().unwrap().len(), 1);
                assert!(outbox.entries().unwrap().is_empty());
            });
    });
}

#[test]
fn hermes_missing_native_skill_is_rejected_before_reservation_or_networking() {
    with_capture_sandbox(|root| {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let mut m = manifest();
                m.enabled_hosts = vec!["hermes".into()];
                m.skills[0].hosts = vec!["hermes".into()];
                let hermes_skill = root.join(".hermes/skills/capture_skill");
                assert!(
                    !hermes_skill.exists(),
                    "precondition: native skill is absent"
                );
                let server = MockServer::start().await;
                let feedback_root = systemprompt_bridge::feedback::metadata_root().unwrap();
                fs::create_dir_all(&feedback_root).unwrap();
                let enrollment = Enrollment::new(
                    &server.uri(),
                    DeviceId::try_new("missing-hermes-device").unwrap(),
                    UserId::new("consumer"),
                    BearerToken::new("sp_device_capture"),
                )
                .unwrap();
                enrollment.save(&feedback_root).unwrap();
                let bearer = BearerToken::default();
                let ctx = context(&m, &server.uri(), &bearer, root);
                assert!(
                    capture_host("hermes", &ctx).await.is_err(),
                    "capture cannot claim a native asset that Hermes does not have"
                );
                assert!(server.received_requests().await.unwrap().is_empty());
                let outbox = Outbox::new(
                    enrollment.outbox_path(&feedback_root),
                    OutboxScope::from_enrollment(&enrollment),
                );
                assert!(outbox.pending_installations().unwrap().is_empty());
                assert!(outbox.entries().unwrap().is_empty());
            });
    });
}
