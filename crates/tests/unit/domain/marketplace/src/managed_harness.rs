// Managed rules and plugins carry their publication identity into the
// catalogue: a published one is stamped, an unpublished one is withheld, and
// an unmanaged key is left alone.

use std::collections::BTreeMap;

use systemprompt_identifiers::{PluginId, UserId};
use systemprompt_manifest::services::ServicesConfig;
use systemprompt_marketplace::CatalogContent;
use systemprompt_marketplace::managed::{
    AssetDigest, ComparisonEvidence, ManagedRepository, NewResource, NewRevision,
    PublicationAction, PublicationRequest, ResourceKind, SnapshotProvenance, SourceSpec,
};
use systemprompt_models::bridge::ids::Sha256Digest;
use systemprompt_models::bridge::manifest::PluginEntry;
use systemprompt_test_fixtures::{ensure_test_bootstrap, seed_user_row, test_db_pool};
use uuid::Uuid;

use crate::managed_resolution::skill_files;

async fn owner_repository() -> (ManagedRepository, UserId) {
    ensure_test_bootstrap();
    let db = test_db_pool().await;
    let owner = UserId::new(format!("managed-harness-{}", Uuid::new_v4()));
    seed_user_row(&db, &owner, &format!("{}@harness.invalid", owner.as_str()))
        .await
        .expect("seed owner");
    (ManagedRepository::new(&db), owner)
}

async fn managed(
    repository: &ManagedRepository,
    owner: &UserId,
    kind: ResourceKind,
    key: &str,
    publish: bool,
) {
    let source = repository
        .register_source(owner, &format!("harness-{key}"), &SourceSpec::Managed)
        .await
        .expect("source");
    let snapshot = repository
        .capture_snapshot(
            owner,
            &source,
            &SnapshotProvenance {
                source_kind: "managed".to_owned(),
                commit: None,
                tree_digest: AssetDigest::of(key.as_bytes()),
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
                upstream_key: key.to_owned(),
                kind,
                resource_key: key.to_owned(),
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
                files: skill_files(key, "# managed\n"),
                dependencies: BTreeMap::new(),
                rationale: "first".to_owned(),
            },
        )
        .await
        .expect("revision");
    if !publish {
        return;
    }
    repository
        .review_and_publish(
            owner,
            owner,
            &PublicationRequest {
                resource_id: resource,
                revision_id: Some(revision),
                action: PublicationAction::InitialAdoption,
                expected_generation: 0,
                operation_key: format!("adopt-{key}-{}", Uuid::new_v4().simple()),
                comparison_evidence: ComparisonEvidence::default(),
                limitations: String::new(),
                cohort: None,
            },
        )
        .await
        .expect("publish");
}

fn write_rule(root: &std::path::Path, id: &str) {
    let dir = root.join("rules").join(id);
    std::fs::create_dir_all(&dir).expect("rule dir");
    std::fs::write(
        dir.join("config.yaml"),
        format!("id: {id}\nname: {id}\ndescription: d\nenabled: true\nfile: index.md\n"),
    )
    .expect("rule config");
    std::fs::write(dir.join("index.md"), "Follow the rule.\n").expect("rule body");
}

fn plugin(id: &str) -> PluginEntry {
    PluginEntry {
        id: PluginId::new(id),
        version: "1.0.0".to_owned(),
        sha256: Sha256Digest::try_new("0".repeat(64)).expect("digest"),
        files: Vec::new(),
        hooks: Default::default(),
        publication: None,
    }
}

#[tokio::test]
async fn published_rules_are_stamped_and_unpublished_ones_withheld() {
    let (repository, owner) = owner_repository().await;
    let suffix = Uuid::new_v4().simple().to_string();
    let published = format!("published_{suffix}");
    let pending = format!("pending_{suffix}");
    let unmanaged = format!("unmanaged_{suffix}");
    managed(&repository, &owner, ResourceKind::Rule, &published, true).await;
    managed(&repository, &owner, ResourceKind::Rule, &pending, false).await;
    let root = tempfile::tempdir().expect("services root");
    for id in [&published, &pending, &unmanaged] {
        write_rule(root.path(), id);
    }
    let catalog = CatalogContent::load(
        &ServicesConfig::default(),
        root.path(),
        "https://api.example.invalid",
    )
    .expect("catalog")
    .with_organization_resources(repository, &owner, &owner)
    .await
    .expect("overlay");
    let (_, rules, ..) = catalog.into_parts();
    let by_id: BTreeMap<&str, _> = rules.iter().map(|r| (r.id.as_str(), r)).collect();
    assert!(by_id[published.as_str()].publication.is_some());
    assert!(!by_id.contains_key(pending.as_str()));
    assert!(by_id[unmanaged.as_str()].publication.is_none());
}

#[tokio::test]
async fn published_plugins_are_stamped_and_unpublished_ones_withheld() {
    let (repository, owner) = owner_repository().await;
    let suffix = Uuid::new_v4().simple().to_string();
    let published = format!("published-{suffix}");
    let pending = format!("pending-{suffix}");
    let unmanaged = format!("unmanaged-{suffix}");
    managed(&repository, &owner, ResourceKind::Plugin, &published, true).await;
    managed(&repository, &owner, ResourceKind::Plugin, &pending, false).await;
    let root = tempfile::tempdir().expect("services root");
    let catalog = CatalogContent::load(
        &ServicesConfig::default(),
        root.path(),
        "https://api.example.invalid",
    )
    .expect("catalog")
    .with_organization_resources(repository, &owner, &owner)
    .await
    .expect("overlay");
    let stamped = catalog.stamp_plugins(vec![
        plugin(&published),
        plugin(&pending),
        plugin(&unmanaged),
    ]);
    let ids: Vec<&str> = stamped.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(ids, vec![published.as_str(), unmanaged.as_str()]);
    let reference = stamped[0].publication.as_ref().expect("publication");
    assert_eq!(reference.generation, 1);
    assert!(stamped[1].publication.is_none());
}
