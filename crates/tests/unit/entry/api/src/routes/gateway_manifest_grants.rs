//! The bridge manifest records a consumer grant for every published
//! organisation skill it hands out, and only for those.
//!
//! Catalogue reach no longer waits for an explicit grant: a published skill
//! that survives the per-user marketplace filter is granted by the fetch
//! itself, so runtime resolution (which still demands an active grant) sees
//! exactly what the manifest delivered. A skill the filter dropped never
//! reached the user and must leave no grant behind.

use std::collections::{BTreeMap, HashSet};
use std::sync::{Arc, OnceLock};

use axum::body::{Body, to_bytes};
use axum::http::{HeaderMap, Request, StatusCode, header};
use systemprompt_api::routes::gateway::bridge_manifest;
use systemprompt_api::services::middleware::{JtiRevocationChecker, JwtContextExtractor};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{DeviceCertId, ManagedResourceId, UserId};
use systemprompt_marketplace::managed::{
    AssetDigest, AssetFile, ManagedRepository, NewResource, NewRevision, PublicationAction,
    PublicationRequest, ResourceKind, RevisionFiles, SnapshotProvenance, SourceSpec,
};
use systemprompt_marketplace::{
    AllowAllFilter, EntryKeepSets, MarketplaceCandidate, MarketplaceFilter, MarketplaceFilterError,
};
use systemprompt_models::bridge::manifest::SignedManifest;
use systemprompt_models::feedback::receipts::{
    ConsumerReceiptRequest, RuntimeFileReadback, SessionBindingRequest,
};
use systemprompt_models::feedback::{ContentDigest, EvaluatorClient};
use systemprompt_models::profile::PathsConfig;
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{
    TestBootstrap, fixture_app_context_with, fixture_db_pool, init_isolated_bootstrap,
    install_test_signing_key, seed_bridge_credential, seed_user_row,
};
use systemprompt_traits::AppContext as _;
use tower::ServiceExt;

// One enabled plugin that claims every skill on the instance, so a managed
// skill published under the system admin is plugin-owned and survives the
// plugin gate in candidate assembly.
const SERVICES_CONFIG: &str = r#"plugins:
  grants-fixture-plugin:
    id: grants-fixture-plugin
    name: Grants fixture plugin
    description: Claims every instance skill for the manifest grant tests.
    version: "1.0.0"
    enabled: true
    author:
      name: test
      email: test@example.com
    keywords: []
    license: BSL-1.0
    category: test
    skills:
      source: instance
    agents:
      source: explicit
"#;

// The bootstrap owns the tempdir holding the services config and the profile
// the handler reads on every request, so it has to outlive the test.
static BOOT: OnceLock<TestBootstrap> = OnceLock::new();

fn boot() -> &'static TestBootstrap {
    BOOT.get_or_init(|| init_isolated_bootstrap("http://127.0.0.1", SERVICES_CONFIG))
}

fn boot_paths(boot: &TestBootstrap) -> PathsConfig {
    PathsConfig {
        system: boot.system_path.display().to_string(),
        services: boot.services_path.display().to_string(),
        bin: boot.bin_path.display().to_string(),
        web_path: Some(boot.system_path.join("web").display().to_string()),
        storage: Some(boot.storage_path.display().to_string()),
        geoip_database: None,
    }
}

// Keeps everything except skills, standing in for a per-user marketplace
// filter that withholds every skill from this consumer.
#[derive(Debug)]
struct DropSkillsFilter;

#[async_trait::async_trait]
impl MarketplaceFilter for DropSkillsFilter {
    async fn filter(
        &self,
        _user_id: &UserId,
        mut candidate: MarketplaceCandidate,
    ) -> Result<MarketplaceCandidate, MarketplaceFilterError> {
        let keep = EntryKeepSets {
            plugins: candidate.plugins.iter().map(|p| p.id.clone()).collect(),
            skills: HashSet::new(),
            agents: candidate.agents.iter().map(|a| a.id.clone()).collect(),
            hooks: candidate.hooks.iter().map(|h| h.id.clone()).collect(),
            mcp_servers: candidate
                .managed_mcp_servers
                .iter()
                .map(|m| m.id.clone())
                .collect(),
            marketplaces: candidate
                .marketplaces
                .iter()
                .map(|m| m.id.clone())
                .collect(),
        };
        candidate.retain_entries(&keep);
        Ok(candidate)
    }
}

struct Harness {
    pool: DbPool,
    ctx: AppContext,
    extractor: Arc<JwtContextExtractor>,
}

async fn harness(filter: Arc<dyn MarketplaceFilter>) -> Harness {
    let boot = boot();
    install_test_signing_key();
    let pool = fixture_db_pool(&boot.database_url)
        .await
        .expect("test database");
    let ctx = fixture_app_context_with(&pool, &boot.database_url, boot_paths(boot), filter)
        .expect("fixture context");
    let owner = ctx.system_admin().id();
    seed_user_row(&pool, owner, &format!("{owner}@manifest-grants.invalid"))
        .await
        .expect("seed the organisation owner");
    let extractor = Arc::new(JwtContextExtractor::new(
        ctx.session_provider().expect("session provider"),
        ctx.user_provider().expect("user provider"),
        JtiRevocationChecker::from_repository(ctx.oauth_repositories().oauth.clone()),
    ));
    Harness {
        pool,
        ctx: (*ctx).clone(),
        extractor,
    }
}

fn skill_files_with_hosts(key: &str, hosts: &[&str]) -> RevisionFiles {
    let mut files = BTreeMap::new();
    let hosts = hosts
        .iter()
        .map(|host| format!("  - {host}\n"))
        .collect::<String>();
    files.insert(
        "config.yaml".to_owned(),
        AssetFile {
            bytes: format!(
                "id: {key}\nname: {key}\ndescription: managed\nenabled: true\nhosts:\n{hosts}"
            )
            .into_bytes(),
            media_type: "application/yaml".to_owned(),
            executable: false,
        },
    );
    files.insert(
        "index.md".to_owned(),
        AssetFile {
            bytes: b"# managed instructions\n".to_vec(),
            media_type: "text/markdown".to_owned(),
            executable: false,
        },
    );
    RevisionFiles(files)
}

// Publishes one managed skill under the organisation owner and returns its
// key and resource id.
async fn publish_organisation_skill(
    repository: &ManagedRepository,
    owner: &UserId,
) -> (String, ManagedResourceId) {
    publish_organisation_skill_with_hosts(repository, owner, &[]).await
}

async fn publish_organisation_skill_with_hosts(
    repository: &ManagedRepository,
    owner: &UserId,
    hosts: &[&str],
) -> (String, ManagedResourceId) {
    let key = format!("skill_{}", uuid::Uuid::new_v4().simple());
    let source = repository
        .register_source(owner, "authoring", &SourceSpec::Managed)
        .await
        .expect("source");
    let snapshot = repository
        .capture_snapshot(
            owner,
            &source,
            &SnapshotProvenance {
                source_kind: "managed".to_owned(),
                commit: None,
                tree_digest: AssetDigest::of(b"tree"),
                importer_version: "test".to_owned(),
            },
        )
        .await
        .expect("snapshot");
    let resource = repository
        .bind_resource(
            owner,
            &NewResource {
                source_id: source,
                upstream_key: key.clone(),
                kind: ResourceKind::Skill,
                resource_key: key.clone(),
            },
        )
        .await
        .expect("resource");
    let revision = repository
        .create_revision(
            owner,
            &NewRevision {
                resource_id: resource.clone(),
                snapshot_id: snapshot,
                parent_id: None,
                files: skill_files_with_hosts(&key, hosts),
                dependencies: BTreeMap::new(),
                rationale: "first revision".to_owned(),
            },
        )
        .await
        .expect("revision");
    repository
        .review_and_publish(
            owner,
            owner,
            &PublicationRequest {
                resource_id: resource.clone(),
                revision_id: Some(revision),
                action: PublicationAction::InitialAdoption,
                expected_generation: 0,
                comparison_evidence: systemprompt_marketplace::managed::ComparisonEvidence::default(
                ),
                limitations: String::new(),
                operation_key: format!("adopt-{key}"),
            },
        )
        .await
        .expect("publish");
    (key, resource)
}

async fn fetch_manifest(harness: &Harness, jwt: &str) -> SignedManifest {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        format!("Bearer {jwt}").parse().expect("bearer header"),
    );
    let axum::Json(envelope) =
        bridge_manifest::manifest(Arc::clone(&harness.extractor), harness.ctx.clone(), headers)
            .await
            .expect("a signed-in consumer receives a manifest");
    serde_json::from_str(&envelope.payload).expect("the payload is the canonical manifest")
}

async fn grant_row(
    pool: &DbPool,
    owner: &UserId,
    resource: &ManagedResourceId,
    consumer: &UserId,
) -> Option<bool> {
    let inner = pool.pool_arc().expect("read pool");
    sqlx::query_scalar::<_, bool>(
        "SELECT revoked_at IS NULL FROM managed_consumer_grants WHERE owner_id=$1 AND \
         resource_id=$2 AND consumer_id=$3",
    )
    .bind(owner.as_str())
    .bind(resource.as_str())
    .bind(consumer.as_str())
    .fetch_optional(inner.as_ref())
    .await
    .expect("grant lookup")
}

#[tokio::test]
async fn manifest_fetch_grants_every_published_skill_it_delivers() {
    let harness = harness(Arc::new(AllowAllFilter)).await;
    let owner = harness.ctx.system_admin().id().clone();
    let repository = harness.ctx.managed_repository();
    let (key, resource) = publish_organisation_skill(repository, &owner).await;
    let consumer = seed_bridge_credential(&harness.pool, "manifest-grants@example.invalid")
        .await
        .expect("consumer credential");
    assert_eq!(
        grant_row(&harness.pool, &owner, &resource, &consumer.user_id).await,
        None,
        "no grant exists before the consumer fetches a manifest"
    );

    let manifest = fetch_manifest(&harness, consumer.jwt.as_str()).await;

    let delivered = manifest
        .skills
        .iter()
        .find(|skill| skill.id.as_str() == key)
        .expect("the published organisation skill reaches an ungranted consumer");
    let publication = delivered
        .publication
        .as_ref()
        .expect("a managed skill carries its publication");
    assert_eq!(publication.resource_id, resource);
    assert_eq!(
        grant_row(&harness.pool, &owner, &resource, &consumer.user_id).await,
        Some(true),
        "delivering the skill records an active grant"
    );
    for skill in manifest.skills.iter().filter(|s| s.publication.is_some()) {
        let resource = &skill.publication.as_ref().expect("publication").resource_id;
        assert_eq!(
            grant_row(&harness.pool, &owner, resource, &consumer.user_id).await,
            Some(true),
            "every delivered publication is granted: {}",
            skill.id
        );
    }
}

#[tokio::test]
async fn manifest_fetch_records_no_grant_for_a_skill_the_filter_dropped() {
    let harness = harness(Arc::new(DropSkillsFilter)).await;
    let owner = harness.ctx.system_admin().id().clone();
    let repository = harness.ctx.managed_repository();
    let (key, resource) = publish_organisation_skill(repository, &owner).await;
    let consumer = seed_bridge_credential(&harness.pool, "manifest-filtered@example.invalid")
        .await
        .expect("consumer credential");

    let manifest = fetch_manifest(&harness, consumer.jwt.as_str()).await;

    assert!(
        manifest.skills.iter().all(|skill| skill.id.as_str() != key),
        "the filter withholds the skill from this consumer"
    );
    assert_eq!(
        grant_row(&harness.pool, &owner, &resource, &consumer.user_id).await,
        None,
        "a skill the consumer never received leaves no grant behind"
    );
}

async fn consumer_token(
    harness: &Harness,
    consumer: &UserId,
    label: &str,
) -> (DeviceCertId, String) {
    let cert = DeviceCertId::generate();
    let writer = harness.pool.write_pool_arc().expect("write pool");
    sqlx::query("INSERT INTO user_device_certs(id,user_id,fingerprint,label) VALUES($1,$2,$3,$4)")
        .bind(cert.as_str())
        .bind(consumer.as_str())
        .bind(cert.as_str())
        .bind(label)
        .execute(writer.as_ref())
        .await
        .expect("device");
    let credential = harness
        .ctx
        .managed_repository()
        .issue_consumer_credential(&cert)
        .await
        .expect("device credential")
        .credential;
    (cert, credential)
}

async fn consumer_bundle_status(
    harness: &Harness,
    resource: &ManagedResourceId,
    publication: &str,
    host: &str,
    authorization: Option<&str>,
) -> StatusCode {
    let mut request = Request::builder().uri(format!(
        "/consumer/resources/{resource}/publications/{publication}/bundle?host={host}"
    ));
    if let Some(value) = authorization {
        request = request.header("authorization", value);
    }
    systemprompt_api::routes::managed::consumer::router()
        .with_state(harness.ctx.clone())
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap()
        .status()
}

#[tokio::test]
async fn consumer_bundle_rejects_missing_and_malformed_credentials_before_granting() {
    let harness = harness(Arc::new(AllowAllFilter)).await;
    let owner = harness.ctx.system_admin().id().clone();
    let (_, resource) = publish_organisation_skill(harness.ctx.managed_repository(), &owner).await;
    let publication: String = sqlx::query_scalar(
        "SELECT id FROM managed_publications WHERE owner_id=$1 AND resource_id=$2 ORDER BY created_at DESC LIMIT 1",
    )
    .bind(owner.as_str()).bind(resource.as_str())
    .fetch_one(harness.pool.pool_arc().unwrap().as_ref()).await.unwrap();

    for authorization in [
        None,
        Some("Basic abc"),
        Some("Bearer ordinary-token"),
        Some("Bearer sp_device_"),
    ] {
        assert_eq!(
            consumer_bundle_status(&harness, &resource, &publication, "codex", authorization).await,
            StatusCode::UNAUTHORIZED
        );
    }
    let grants: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM managed_consumer_grants WHERE owner_id=$1 AND resource_id=$2",
    )
    .bind(owner.as_str())
    .bind(resource.as_str())
    .fetch_one(harness.pool.pool_arc().unwrap().as_ref())
    .await
    .unwrap();
    assert_eq!(grants, 0, "unauthenticated requests cannot persist a grant");
}

#[tokio::test]
async fn revoked_consumer_credential_cannot_retain_a_catalog_grant() {
    let harness = harness(Arc::new(AllowAllFilter)).await;
    let owner = harness.ctx.system_admin().id().clone();
    let (_, resource) = publish_organisation_skill(harness.ctx.managed_repository(), &owner).await;
    let consumer = seed_bridge_credential(&harness.pool, "revoked-consumer@example.invalid")
        .await
        .unwrap();
    let (cert, token) = consumer_token(&harness, &consumer.user_id, "revoked consumer").await;
    let publication: String = sqlx::query_scalar("SELECT id FROM managed_publications WHERE owner_id=$1 AND resource_id=$2 ORDER BY created_at DESC LIMIT 1")
        .bind(owner.as_str()).bind(resource.as_str()).fetch_one(harness.pool.pool_arc().unwrap().as_ref()).await.unwrap();
    harness
        .ctx
        .managed_repository()
        .revoke_consumer_credential(&cert)
        .await
        .unwrap();

    assert_eq!(
        consumer_bundle_status(
            &harness,
            &resource,
            &publication,
            "codex",
            Some(&format!("Bearer {token}"))
        )
        .await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        grant_row(&harness.pool, &owner, &resource, &consumer.user_id).await,
        None
    );
}

#[tokio::test]
async fn unknown_resource_id_cannot_be_used_to_mint_a_catalog_grant() {
    let harness = harness(Arc::new(AllowAllFilter)).await;
    let consumer = seed_bridge_credential(&harness.pool, "unknown-resource@example.invalid")
        .await
        .unwrap();
    let (_, token) = consumer_token(&harness, &consumer.user_id, "unknown resource").await;
    let resource = ManagedResourceId::generate();

    assert_eq!(
        consumer_bundle_status(
            &harness,
            &resource,
            "missing-publication",
            "codex",
            Some(&format!("Bearer {token}"))
        )
        .await,
        StatusCode::FORBIDDEN
    );
    let grants: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM managed_consumer_grants WHERE resource_id=$1 AND consumer_id=$2",
    )
    .bind(resource.as_str())
    .bind(consumer.user_id.as_str())
    .fetch_one(harness.pool.pool_arc().unwrap().as_ref())
    .await
    .unwrap();
    assert_eq!(grants, 0);
}

#[tokio::test]
async fn filtered_resource_cannot_be_recovered_by_guessing_its_bundle_url() {
    let harness = harness(Arc::new(DropSkillsFilter)).await;
    let owner = harness.ctx.system_admin().id().clone();
    let (_, resource) = publish_organisation_skill(harness.ctx.managed_repository(), &owner).await;
    let consumer = seed_bridge_credential(&harness.pool, "filtered-bundle@example.invalid")
        .await
        .unwrap();
    let (_, token) = consumer_token(&harness, &consumer.user_id, "filtered bundle").await;
    let publication: String = sqlx::query_scalar("SELECT id FROM managed_publications WHERE owner_id=$1 AND resource_id=$2 ORDER BY created_at DESC LIMIT 1")
        .bind(owner.as_str()).bind(resource.as_str()).fetch_one(harness.pool.pool_arc().unwrap().as_ref()).await.unwrap();

    assert_eq!(
        consumer_bundle_status(
            &harness,
            &resource,
            &publication,
            "codex",
            Some(&format!("Bearer {token}"))
        )
        .await,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        grant_row(&harness.pool, &owner, &resource, &consumer.user_id).await,
        None
    );
}

#[tokio::test]
async fn consumer_bundle_enforces_host_scope_without_changing_the_catalog_grant() {
    let harness = harness(Arc::new(AllowAllFilter)).await;
    let owner = harness.ctx.system_admin().id().clone();
    let repository = harness.ctx.managed_repository();
    let (key, resource) =
        publish_organisation_skill_with_hosts(repository, &owner, &["claude-code"]).await;
    let consumer = seed_bridge_credential(&harness.pool, "host-scoped-bundle@example.invalid")
        .await
        .expect("consumer");
    let manifest = fetch_manifest(&harness, consumer.jwt.as_str()).await;
    assert!(
        manifest.skills.iter().any(|skill| skill.id.as_str() == key),
        "the authorised catalog advertises the host-scoped skill"
    );
    assert_eq!(
        grant_row(&harness.pool, &owner, &resource, &consumer.user_id).await,
        Some(true),
        "catalog authorisation records the user-level grant before host filtering"
    );
    let (_, token) = consumer_token(&harness, &consumer.user_id, "host-scoped bundle").await;
    let publication: String = sqlx::query_scalar(
        "SELECT id FROM managed_publications WHERE owner_id=$1 AND resource_id=$2 ORDER BY created_at DESC LIMIT 1",
    )
    .bind(owner.as_str())
    .bind(resource.as_str())
    .fetch_one(harness.pool.pool_arc().expect("read pool").as_ref())
    .await
    .expect("publication");
    let authorization = format!("Bearer {token}");

    assert_eq!(
        consumer_bundle_status(
            &harness,
            &resource,
            &publication,
            "codex",
            Some(&authorization),
        )
        .await,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        grant_row(&harness.pool, &owner, &resource, &consumer.user_id).await,
        Some(true),
        "a rejected host request must not alter the existing catalog grant"
    );

    let response = systemprompt_api::routes::managed::consumer::router()
        .with_state(harness.ctx.clone())
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/consumer/resources/{resource}/publications/{publication}/bundle?host=claude-code"
                ))
                .header("authorization", authorization)
                .body(Body::empty())
                .expect("allowed-host bundle request"),
        )
        .await
        .expect("bundle response");
    assert_eq!(response.status(), StatusCode::OK);
    let plan: systemprompt_models::feedback::receipts::ConsumerInstallationPlan =
        serde_json::from_slice(
            &to_bytes(response.into_body(), 1_000_000)
                .await
                .expect("bundle body"),
        )
        .expect("bundle plan");
    assert_eq!(plan.resource_id, resource);
    assert_eq!(plan.publication_id.as_str(), publication);
    assert!(
        plan.runtime_files
            .iter()
            .any(|file| file.path == "SKILL.md")
    );
    assert_eq!(
        grant_row(&harness.pool, &owner, &resource, &consumer.user_id).await,
        Some(true),
        "the allowed host receives the retained grant with its plan"
    );
}

#[tokio::test]
async fn consumer_http_flow_binds_receipts_and_refuses_another_device() {
    let harness = harness(Arc::new(AllowAllFilter)).await;
    let owner = harness.ctx.system_admin().id().clone();
    let repository = harness.ctx.managed_repository();
    let (_, resource) = publish_organisation_skill(repository, &owner).await;
    let consumer = seed_bridge_credential(&harness.pool, "consumer-http@example.invalid")
        .await
        .expect("consumer");
    let cert = DeviceCertId::generate();
    let writer = harness.pool.write_pool_arc().expect("write pool");
    sqlx::query("INSERT INTO user_device_certs(id,user_id,fingerprint,label) VALUES($1,$2,$3,'consumer HTTP')")
        .bind(cert.as_str()).bind(consumer.user_id.as_str()).bind(cert.as_str())
        .execute(writer.as_ref()).await.expect("device");
    let credential = repository
        .issue_consumer_credential(&cert)
        .await
        .expect("credential");
    let publication: String = sqlx::query_scalar("SELECT id FROM managed_publications WHERE owner_id=$1 AND resource_id=$2 ORDER BY created_at DESC LIMIT 1")
        .bind(owner.as_str()).bind(resource.as_str()).fetch_one(writer.as_ref()).await.expect("publication");
    let router =
        systemprompt_api::routes::managed::consumer::router().with_state(harness.ctx.clone());
    let bundle = router
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/consumer/resources/{resource}/publications/{publication}/bundle?host=codex"
                ))
                .header("authorization", format!("Bearer {}", credential.credential))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(bundle.status(), StatusCode::OK);
    let plan: systemprompt_models::feedback::receipts::ConsumerInstallationPlan =
        serde_json::from_slice(&to_bytes(bundle.into_body(), 1_000_000).await.unwrap()).unwrap();
    assert_eq!(plan.resource_id, resource);
    let runtime_files: Vec<_> = plan
        .runtime_files
        .iter()
        .map(|file| RuntimeFileReadback {
            path: file.path.clone(),
            digest: ContentDigest::of(&file.bytes),
            bytes: file.bytes.len() as u64,
            executable: file.executable,
            content_check: systemprompt_models::feedback::receipts::ReadbackStatus::Verified,
            mode_check: systemprompt_models::feedback::receipts::ReadbackStatus::Verified,
        })
        .collect();
    let files = plan
        .canonical_files
        .iter()
        .map(|canonical| {
            let source_path = format!(
                ".systemprompt-source/{}/{}",
                canonical.revision_id, canonical.path
            );
            let runtime = runtime_files
                .iter()
                .find(|file| file.path == source_path)
                .expect("bundle carries every canonical source file");
            assert_eq!(runtime.digest, canonical.digest);
            assert_eq!(runtime.bytes, canonical.bytes);
            assert_eq!(runtime.executable, canonical.executable);
            let mut verified = canonical.clone();
            verified.content_check =
                systemprompt_models::feedback::receipts::ReadbackStatus::Verified;
            verified.mode_check = systemprompt_models::feedback::receipts::ReadbackStatus::Verified;
            verified
        })
        .collect();
    let receipt = ConsumerReceiptRequest {
        installation_id: systemprompt_identifiers::ConsumerInstallationId::generate(),
        publication_id: plan.publication_id.clone(),
        resource_id: plan.resource_id.clone(),
        revision_id: plan.revision_id.clone(),
        generation: plan.generation,
        bundle_digest: plan.bundle_digest.clone(),
        host: EvaluatorClient::Codex,
        observed_at: chrono::Utc::now(),
        files,
        runtime_files,
    };
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/consumer/receipts")
                .header("authorization", format!("Bearer {}", credential.credential))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&receipt).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let recorded: systemprompt_models::feedback::receipts::ConsumerReceiptResponse =
        serde_json::from_slice(&to_bytes(response.into_body(), 1_000_000).await.unwrap()).unwrap();
    assert!(recorded.fully_verified);
    repository
        .reconcile_inventory(&owner, &[])
        .await
        .expect("managed inventory projection");
    repository
        .refresh_installation_coverage(&owner)
        .await
        .expect("installation coverage refresh");
    let coverage = repository
        .installation_coverage(&owner, &resource)
        .await
        .expect("installation coverage")
        .expect("coverage row for the published resource");
    assert_eq!(coverage.eligible_devices, 1);
    assert_eq!(coverage.current_acknowledged_devices, 1);
    assert_eq!(coverage.current_verified_devices, 1);
    assert_eq!(coverage.acknowledged_installations, 1);
    let binding_request = SessionBindingRequest {
        receipt_id: recorded.receipt_id.clone(),
        host: EvaluatorClient::Codex,
        session_id: systemprompt_identifiers::NativeSessionId::new("consumer-http-session"),
    };
    let bound = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/consumer/session-bindings")
                .header("authorization", format!("Bearer {}", credential.credential))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&binding_request).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(bound.status(), StatusCode::OK);
    let binding: systemprompt_marketplace::managed::consumer::ConsumerSessionBinding =
        serde_json::from_slice(&to_bytes(bound.into_body(), 1_000_000).await.unwrap()).unwrap();
    assert!(binding.bound_at <= chrono::Utc::now());
    let foreign = seed_bridge_credential(&harness.pool, "consumer-http-foreign@example.invalid")
        .await
        .expect("foreign");
    let foreign_cert = DeviceCertId::generate();
    sqlx::query("INSERT INTO user_device_certs(id,user_id,fingerprint,label) VALUES($1,$2,$3,'foreign HTTP')").bind(foreign_cert.as_str()).bind(foreign.user_id.as_str()).bind(foreign_cert.as_str()).execute(writer.as_ref()).await.expect("foreign device");
    let foreign_token = repository
        .issue_consumer_credential(&foreign_cert)
        .await
        .expect("foreign credential");
    let hidden = router
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/consumer/session-bindings")
                .header(
                    "authorization",
                    format!("Bearer {}", foreign_token.credential),
                )
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&binding_request).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(hidden.status(), StatusCode::FORBIDDEN);
    let hidden: serde_json::Value =
        serde_json::from_slice(&to_bytes(hidden.into_body(), 1_000_000).await.unwrap()).unwrap();
    let rendered = hidden.to_string();
    assert!(!rendered.contains(recorded.receipt_id.as_str()));
    assert!(!rendered.contains(resource.as_str()));
    assert!(!rendered.contains(binding.id.as_str()));
}
struct PrivateConsumer {
    database: systemprompt_test_fixtures::DisposableDb,
    harness: Harness,
    token: String,
    request: ConsumerReceiptRequest,
}

async fn private_consumer(label: &str) -> PrivateConsumer {
    let database = systemprompt_test_fixtures::DisposableDb::installed(label)
        .await
        .unwrap();
    let pool = database.pool().await.unwrap();
    let boot = boot();
    let ctx = fixture_app_context_with(
        &pool,
        database.url(),
        boot_paths(boot),
        Arc::new(AllowAllFilter),
    )
    .unwrap();
    let owner = ctx.system_admin().id().clone();
    seed_user_row(&pool, &owner, &format!("{owner}@private-consumer.invalid"))
        .await
        .unwrap();
    let extractor = Arc::new(JwtContextExtractor::new(
        ctx.session_provider().unwrap(),
        ctx.user_provider().unwrap(),
        JtiRevocationChecker::from_repository(ctx.oauth_repositories().oauth.clone()),
    ));
    let harness = Harness {
        pool,
        ctx: (*ctx).clone(),
        extractor,
    };
    let (_, resource) = publish_organisation_skill(harness.ctx.managed_repository(), &owner).await;
    let consumer = seed_bridge_credential(&harness.pool, &format!("{label}@example.invalid"))
        .await
        .unwrap();
    let manifest = fetch_manifest(&harness, consumer.jwt.as_str()).await;
    let publication = manifest
        .skills
        .iter()
        .find_map(|skill| {
            skill
                .publication
                .as_ref()
                .filter(|p| p.resource_id == resource)
        })
        .unwrap()
        .clone();
    let (_, token) = consumer_token(&harness, &consumer.user_id, label).await;
    let plan = harness
        .ctx
        .managed_repository()
        .consumer_installation_plan(
            &token,
            &resource,
            &publication.publication_id,
            EvaluatorClient::Codex,
        )
        .await
        .unwrap();
    let runtime_files = plan
        .runtime_files
        .iter()
        .map(|file| RuntimeFileReadback {
            path: file.path.clone(),
            digest: ContentDigest::of(&file.bytes),
            bytes: file.bytes.len() as u64,
            executable: file.executable,
            content_check: systemprompt_models::feedback::receipts::ReadbackStatus::Verified,
            mode_check: systemprompt_models::feedback::receipts::ReadbackStatus::Verified,
        })
        .collect::<Vec<_>>();
    let files = plan
        .canonical_files
        .iter()
        .map(|file| {
            let mut f = file.clone();
            f.content_check = systemprompt_models::feedback::receipts::ReadbackStatus::Verified;
            f.mode_check = systemprompt_models::feedback::receipts::ReadbackStatus::Verified;
            f
        })
        .collect();
    let request = ConsumerReceiptRequest {
        installation_id: systemprompt_identifiers::ConsumerInstallationId::generate(),
        publication_id: plan.publication_id,
        resource_id: plan.resource_id,
        revision_id: plan.revision_id,
        generation: plan.generation,
        bundle_digest: plan.bundle_digest,
        host: EvaluatorClient::Codex,
        observed_at: chrono::Utc::now(),
        files,
        runtime_files,
    };
    PrivateConsumer {
        database,
        harness,
        token,
        request,
    }
}


fn consumer_router(f: &PrivateConsumer) -> axum::Router {
    systemprompt_api::routes::managed::consumer::router().with_state(f.harness.ctx.clone())
}
async fn post_json(
    app: axum::Router,
    path: &str,
    token: &str,
    value: &impl serde::Serialize,
) -> axum::response::Response {
    app.oneshot(
        Request::builder()
            .method("POST")
            .uri(path)
            .header("authorization", format!("Bearer {token}"))
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(value).unwrap()))
            .unwrap(),
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn receipt_storage_failure_leaves_no_partial_evidence_and_retry_commits_once() {
    let f = private_consumer("receipt_write_recovery").await;
    let db = f.harness.pool.pool_arc().unwrap();
    sqlx::query("CREATE FUNCTION reject_receipt() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'owned write failure'; END $$").execute(db.as_ref()).await.unwrap();
    sqlx::query("CREATE TRIGGER reject_receipt BEFORE INSERT ON managed_installation_receipts FOR EACH ROW EXECUTE FUNCTION reject_receipt()").execute(db.as_ref()).await.unwrap();
    let failed = post_json(
        consumer_router(&f),
        "/consumer/receipts",
        &f.token,
        &f.request,
    )
    .await;
    assert_eq!(failed.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM managed_installation_receipts WHERE installation_id=$1",
    )
    .bind(f.request.installation_id.as_str())
    .fetch_one(db.as_ref())
    .await
    .unwrap();
    assert_eq!(count, 0);
    sqlx::query("DROP TRIGGER reject_receipt ON managed_installation_receipts")
        .execute(db.as_ref())
        .await
        .unwrap();
    sqlx::query("DROP FUNCTION reject_receipt()")
        .execute(db.as_ref())
        .await
        .unwrap();
    let recovered = post_json(
        consumer_router(&f),
        "/consumer/receipts",
        &f.token,
        &f.request,
    )
    .await;
    assert_eq!(recovered.status(), StatusCode::OK);
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM managed_installation_receipts WHERE installation_id=$1",
    )
    .bind(f.request.installation_id.as_str())
    .fetch_one(db.as_ref())
    .await
    .unwrap();
    assert_eq!(count, 1);
    f.database.drop_now().await;
}

#[tokio::test]
async fn invalid_runtime_readback_is_rejected_without_receipt_and_corrected_retry_succeeds() {
    let f = private_consumer("receipt_readback_recovery").await;
    let db = f.harness.pool.pool_arc().unwrap();
    let mut invalid = f.request.clone();
    invalid.runtime_files[0].digest = ContentDigest::of(b"wrong bytes");
    assert_eq!(
        post_json(
            consumer_router(&f),
            "/consumer/receipts",
            &f.token,
            &invalid
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM managed_installation_receipts WHERE installation_id=$1",
    )
    .bind(f.request.installation_id.as_str())
    .fetch_one(db.as_ref())
    .await
    .unwrap();
    assert_eq!(count, 0);
    assert_eq!(
        post_json(
            consumer_router(&f),
            "/consumer/receipts",
            &f.token,
            &f.request
        )
        .await
        .status(),
        StatusCode::OK
    );
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM managed_installation_receipts WHERE installation_id=$1",
    )
    .bind(f.request.installation_id.as_str())
    .fetch_one(db.as_ref())
    .await
    .unwrap();
    assert_eq!(count, 1, "the corrected retry commits exactly one receipt");
    f.database.drop_now().await;
}

#[tokio::test]
async fn session_binding_storage_failure_leaves_no_binding_and_retry_is_durable() {
    let f = private_consumer("binding_write_recovery").await;
    let db = f.harness.pool.pool_arc().unwrap();
    let receipt = post_json(
        consumer_router(&f),
        "/consumer/receipts",
        &f.token,
        &f.request,
    )
    .await;
    let recorded: systemprompt_models::feedback::receipts::ConsumerReceiptResponse =
        serde_json::from_slice(&to_bytes(receipt.into_body(), 1_000_000).await.unwrap()).unwrap();
    let binding = SessionBindingRequest {
        receipt_id: recorded.receipt_id,
        host: EvaluatorClient::Codex,
        session_id: systemprompt_identifiers::NativeSessionId::new("binding-write-recovery"),
    };
    sqlx::query("CREATE FUNCTION reject_binding() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'owned write failure'; END $$").execute(db.as_ref()).await.unwrap();
    sqlx::query("CREATE TRIGGER reject_binding BEFORE INSERT ON managed_consumer_session_bindings FOR EACH ROW EXECUTE FUNCTION reject_binding()").execute(db.as_ref()).await.unwrap();
    assert_eq!(
        post_json(
            consumer_router(&f),
            "/consumer/session-bindings",
            &f.token,
            &binding
        )
        .await
        .status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM managed_consumer_session_bindings WHERE native_session_id=$1",
    )
    .bind(binding.session_id.as_str())
    .fetch_one(db.as_ref())
    .await
    .unwrap();
    assert_eq!(count, 0);
    sqlx::query("DROP TRIGGER reject_binding ON managed_consumer_session_bindings")
        .execute(db.as_ref())
        .await
        .unwrap();
    sqlx::query("DROP FUNCTION reject_binding()")
        .execute(db.as_ref())
        .await
        .unwrap();
    let recovered = post_json(
        consumer_router(&f),
        "/consumer/session-bindings",
        &f.token,
        &binding,
    )
    .await;
    assert_eq!(recovered.status(), StatusCode::OK);
    let recovered: systemprompt_marketplace::managed::consumer::ConsumerSessionBinding =
        serde_json::from_slice(&to_bytes(recovered.into_body(), 1_000_000).await.unwrap()).unwrap();
    let row: (String, String) = sqlx::query_as(
        "SELECT receipt_id,native_session_id FROM managed_consumer_session_bindings WHERE id=$1",
    )
    .bind(recovered.id.as_str())
    .fetch_one(db.as_ref())
    .await
    .unwrap();
    assert_eq!(row.0, binding.receipt_id.as_str());
    assert_eq!(row.1, binding.session_id.as_str());
    f.database.drop_now().await;
}
