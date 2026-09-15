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

use axum::http::{HeaderMap, header};
use systemprompt_api::routes::gateway::bridge_manifest;
use systemprompt_api::services::middleware::{JtiRevocationChecker, JwtContextExtractor};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{ManagedResourceId, UserId};
use systemprompt_marketplace::managed::{
    AssetDigest, AssetFile, ManagedRepository, NewResource, NewRevision, PublicationAction,
    PublicationRequest, ResourceKind, RevisionFiles, SnapshotProvenance, SourceSpec,
};
use systemprompt_marketplace::{
    AllowAllFilter, EntryKeepSets, MarketplaceCandidate, MarketplaceFilter, MarketplaceFilterError,
};
use systemprompt_models::bridge::manifest::SignedManifest;
use systemprompt_models::profile::PathsConfig;
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{
    TestBootstrap, fixture_app_context_with, fixture_db_pool, init_isolated_bootstrap,
    install_test_signing_key, seed_bridge_credential, seed_user_row,
};
use systemprompt_traits::AppContext as _;

// One enabled plugin that claims every skill on the instance, so a managed
// skill published under the system admin is plugin-owned and survives the
// plugin gate in candidate assembly.
const SERVICES_CONFIG: &str = r#"plugins:
  grants_fixture_plugin:
    id: grants_fixture_plugin
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

fn skill_files(key: &str) -> RevisionFiles {
    let mut files = BTreeMap::new();
    files.insert(
        "config.yaml".to_owned(),
        AssetFile {
            bytes: format!("id: {key}\nname: {key}\ndescription: managed\nenabled: true\n")
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
                files: skill_files(&key),
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
                comparison_evidence: serde_json::json!({}),
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
