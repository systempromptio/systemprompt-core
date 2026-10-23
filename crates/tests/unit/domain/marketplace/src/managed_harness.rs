// Managed rules and plugins carry their publication identity into the
// catalogue: a published one is stamped, an unpublished one is withheld, and
// an unmanaged key is left alone.

use std::collections::BTreeMap;

use systemprompt_identifiers::{PluginId, UserId};
use systemprompt_manifest::services::ServicesConfig;
use systemprompt_marketplace::CatalogContent;
use systemprompt_marketplace::managed::{
    AssetDigest, AssetFile, ComparisonEvidence, ManagedRepository, NewResource, NewRevision,
    PublicationAction, PublicationRequest, ResourceKind, RevisionFiles, SnapshotProvenance,
    SourceSpec,
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
    managed_files(
        repository,
        owner,
        kind,
        key,
        publish,
        skill_files(key, "# managed\n"),
    )
    .await;
}

async fn managed_files(
    repository: &ManagedRepository,
    owner: &UserId,
    kind: ResourceKind,
    key: &str,
    publish: bool,
    files: RevisionFiles,
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
                files,
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

fn files(entries: &[(&str, &str)]) -> RevisionFiles {
    RevisionFiles(
        entries
            .iter()
            .map(|(path, body)| {
                (
                    (*path).to_owned(),
                    AssetFile {
                        bytes: body.as_bytes().to_vec(),
                        media_type: "text/plain".to_owned(),
                        executable: false,
                    },
                )
            })
            .collect(),
    )
}

#[tokio::test]
async fn a_published_rule_is_served_from_its_revision_not_from_disk() {
    let (repository, owner) = owner_repository().await;
    let key = format!("rule_{}", Uuid::new_v4().simple());
    managed_files(
        &repository,
        &owner,
        ResourceKind::Rule,
        &key,
        true,
        files(&[
            (
                "config.yaml",
                &format!("id: {key}\nname: {key}\ndescription: d\nenabled: true\nfile: index.md\n"),
            ),
            ("index.md", "Follow the published rule.\n"),
        ]),
    )
    .await;
    let root = tempfile::tempdir().expect("services root");
    write_rule(root.path(), &key);
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
    let rule = rules.iter().find(|r| r.id.as_str() == key).expect("rule");
    assert_eq!(rule.instructions, "Follow the published rule.");
    assert!(
        rule.file_path.starts_with("managed://"),
        "{}",
        rule.file_path
    );
    assert!(rule.publication.is_some());
}

#[tokio::test]
async fn a_published_plugin_bundle_carries_its_revision_files() {
    let (repository, owner) = owner_repository().await;
    let id = format!("plugin-{}", Uuid::new_v4().simple());
    managed_files(
        &repository,
        &owner,
        ResourceKind::Plugin,
        &id,
        true,
        files(&[(
            "hosts/opencode/plugins/audit.js",
            "export default 'managed';\n",
        )]),
    )
    .await;
    let root = tempfile::tempdir().expect("services root");
    crate::helpers::write_skill_on_disk(root.path(), "shipped_skill");
    let disk_plugin = root
        .path()
        .join("plugins")
        .join(&id)
        .join("hosts/opencode/plugins");
    std::fs::create_dir_all(&disk_plugin).expect("plugin dir");
    std::fs::write(disk_plugin.join("audit.js"), "export default 'disk';\n").expect("disk file");
    std::fs::write(disk_plugin.join("extra.js"), "export default 'extra';\n").expect("disk file");
    let mut market = crate::helpers::marketplace("acme");
    market.plugins = crate::helpers::include(&[id.as_str()]);
    let mut services = crate::helpers::config_with(vec![market]);
    let mut plugin = crate::helpers::plugin_shipping_artifacts(&id, "shipped_skill", &[]);
    plugin.artifacts = Default::default();
    services.plugins.insert(id.clone(), plugin);
    let catalog = CatalogContent::load(&services, root.path(), "https://api.example.invalid")
        .expect("catalog")
        .with_organization_resources(repository, &owner, &owner)
        .await
        .expect("overlay");
    let bundles =
        systemprompt_marketplace::catalog::plugin_bundles(&services, &catalog.as_content())
            .expect("bundles");
    let bundle = bundles.get(&PluginId::new(&id)).expect("plugin bundle");
    assert_eq!(
        bundle["hosts/opencode/plugins/audit.js"].bytes,
        b"export default 'managed';\n".to_vec()
    );
    assert!(
        !bundle.contains_key("hosts/opencode/plugins/extra.js"),
        "a managed plugin serves only its revision's own files"
    );
}
