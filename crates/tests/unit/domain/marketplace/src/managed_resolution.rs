// Managed skill resolution as the runtime sees it: a managed key that is not
// published is withheld (the disk copy is not served either), a published one
// replaces the disk copy, and only corrupt retained content is an error.

use std::collections::BTreeMap;

use systemprompt_identifiers::{ManagedResourceId, UserId};
use systemprompt_marketplace::CatalogContent;
use systemprompt_marketplace::managed::{
    AssetDigest, AssetFile, ManagedRepository, ManagedResourceResolver, ManagedSkillResolution,
    NewResource, NewRevision, PublicationAction, PublicationRequest, ResourceKind, RevisionFiles,
    SnapshotProvenance, SourceSpec,
};
use systemprompt_models::services::ServicesConfig;
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_db_pool, seed_user_row};
use systemprompt_traits::{ManagedSkillResolver, SkillResolution, WithheldReason};
use uuid::Uuid;

pub(super) struct Fixture {
    pub(super) repository: ManagedRepository,
    pub(super) resolver: ManagedResourceResolver,
    pub(super) owner: UserId,
    pub(super) key: String,
    pub(super) resource: ManagedResourceId,
    pub(super) revision: systemprompt_identifiers::ResourceRevisionId,
}

pub(super) fn skill_files(key: &str, body: &str) -> RevisionFiles {
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
            bytes: body.as_bytes().to_vec(),
            media_type: "text/markdown".to_owned(),
            executable: false,
        },
    );
    RevisionFiles(files)
}

pub(super) async fn fixture() -> Option<Fixture> {
    fixture_with_key(format!("skill_{}", Uuid::new_v4().simple())).await
}

pub(super) async fn fixture_with_key(key: String) -> Option<Fixture> {
    let bootstrap = ensure_test_bootstrap();
    let db = fixture_db_pool(&bootstrap.database_url).await.ok()?;
    let owner = UserId::new(format!("managed-res-{}", Uuid::new_v4()));
    seed_user_row(&db, &owner, &format!("{}@managed.invalid", owner.as_str()))
        .await
        .ok()?;
    let repository = ManagedRepository::new(&db).ok()?;
    let source = repository
        .register_source(&owner, "authoring", &SourceSpec::Managed)
        .await
        .expect("source");
    let snapshot = repository
        .capture_snapshot(
            &owner,
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
            &owner,
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
            &owner,
            &NewRevision {
                resource_id: resource.clone(),
                snapshot_id: snapshot,
                parent_id: None,
                files: skill_files(&key, "# managed instructions\n"),
                dependencies: BTreeMap::new(),
                rationale: "first revision".to_owned(),
            },
        )
        .await
        .expect("revision");
    Some(Fixture {
        resolver: ManagedResourceResolver::new(repository.clone()),
        repository,
        owner,
        key,
        resource,
        revision,
    })
}

pub(super) async fn publish(f: &Fixture) {
    f.repository
        .review_and_publish(
            &f.owner,
            &f.owner,
            &PublicationRequest {
                resource_id: f.resource.clone(),
                revision_id: Some(f.revision.clone()),
                action: PublicationAction::InitialAdoption,
                expected_generation: 0,
                operation_key: format!("adopt-{}", f.key),
                comparison_evidence: systemprompt_marketplace::managed::ComparisonEvidence::default(
                ),
                limitations: String::new(),
            },
        )
        .await
        .expect("publish");
}

pub(super) async fn withdraw(f: &Fixture) {
    f.repository
        .review_and_publish(
            &f.owner,
            &f.owner,
            &PublicationRequest {
                resource_id: f.resource.clone(),
                revision_id: None,
                action: PublicationAction::Withdraw,
                expected_generation: 1,
                operation_key: format!("withdraw-{}", f.key),
                comparison_evidence: systemprompt_marketplace::managed::ComparisonEvidence::default(
                ),
                limitations: String::new(),
            },
        )
        .await
        .expect("withdraw");
}

pub(super) fn disk_catalog_with(key: &str) -> (tempfile::TempDir, CatalogContent) {
    let dir = tempfile::tempdir().expect("services root");
    crate::helpers::write_skill_on_disk(dir.path(), key);
    std::fs::write(
        dir.path().join("skills").join(key).join("index.md"),
        "# disk instructions\n",
    )
    .expect("disk skill body");
    let catalog = CatalogContent::load(
        &ServicesConfig::default(),
        dir.path(),
        "https://api.example.invalid",
    )
    .expect("disk catalog");
    (dir, catalog)
}

#[tokio::test]
async fn an_unmanaged_key_falls_back_to_disk() {
    let Some(f) = fixture().await else {
        return;
    };
    let outcome = f
        .resolver
        .resolve_skill(&f.owner, "never_registered")
        .await
        .expect("resolve");
    assert!(matches!(outcome, ManagedSkillResolution::NotManaged));
}

#[tokio::test]
async fn a_never_adopted_managed_skill_is_withheld_and_hides_its_disk_copy() {
    let Some(f) = fixture().await else {
        return;
    };
    let outcome = f
        .resolver
        .resolve_skill(&f.owner, &f.key)
        .await
        .expect("a never-adopted key is a resolution, not an error");
    assert!(matches!(
        outcome,
        ManagedSkillResolution::Withheld(reason) if *reason == WithheldReason::NeverAdopted
    ));

    let (_dir, catalog) = disk_catalog_with(&f.key);
    assert_eq!(catalog.as_content().skills.len(), 1);
    let overlaid = catalog
        .with_managed_skills(f.repository.clone(), &f.owner)
        .await
        .expect("the catalogue survives a withheld skill");
    assert!(
        overlaid.as_content().skills.is_empty(),
        "the disk copy of a managed-but-unpublished skill must not be served"
    );

    let runtime: &dyn ManagedSkillResolver = &f.resolver;
    assert_eq!(
        runtime
            .resolve_skill(&f.owner, &f.key)
            .await
            .expect("runtime"),
        SkillResolution::Withheld(WithheldReason::NeverAdopted)
    );
}

#[tokio::test]
async fn a_published_managed_skill_replaces_its_disk_copy() {
    let Some(f) = fixture().await else {
        return;
    };
    publish(&f).await;

    let (_dir, catalog) = disk_catalog_with(&f.key);
    let overlaid = catalog
        .with_managed_skills(f.repository.clone(), &f.owner)
        .await
        .expect("overlay");
    let skills = &overlaid.as_content().skills;
    assert_eq!(skills.len(), 1);
    assert!(skills[0].instructions.contains("managed instructions"));
    let ManagedSkillResolution::Published(published) = f
        .resolver
        .resolve_skill(&f.owner, &f.key)
        .await
        .expect("resolve")
    else {
        panic!("published skill must resolve");
    };
    assert_eq!(
        skills[0].file_path,
        format!("managed://{}@{}", f.key, published.bundle_digest.as_str())
    );
    let bundle_content = overlaid.as_content();
    let files = bundle_content
        .managed_files
        .get(&skills[0].id)
        .expect("the published revision files ride with the catalogue");
    assert!(files.0.contains_key("index.md"));

    let runtime: &dyn ManagedSkillResolver = &f.resolver;
    let SkillResolution::Published(skill) = runtime
        .resolve_skill(&f.owner, &f.key)
        .await
        .expect("runtime")
    else {
        panic!("published skill must resolve for the runtime");
    };
    assert!(skill.instructions.contains("managed instructions"));
}

#[tokio::test]
async fn a_withdrawn_managed_skill_is_withheld_after_publication() {
    let Some(f) = fixture().await else {
        return;
    };
    publish(&f).await;
    withdraw(&f).await;

    assert!(matches!(
        f.resolver
            .resolve_skill(&f.owner, &f.key)
            .await
            .expect("resolve"),
        ManagedSkillResolution::Withheld(reason) if *reason == WithheldReason::Withdrawn
    ));
    let (_dir, catalog) = disk_catalog_with(&f.key);
    let overlaid = catalog
        .with_managed_skills(f.repository.clone(), &f.owner)
        .await
        .expect("overlay");
    assert!(overlaid.as_content().skills.is_empty());
}
