use std::collections::BTreeMap;
use systemprompt_identifiers::{ManagedResourceId, ResourceRevisionId, TaskId, UserId};
use systemprompt_marketplace::inventory::{
    BaselinePreparation, BaselineScope, InventoryService, ObservedMembership, configured_identity,
    scan_configured_inventory,
};
use systemprompt_marketplace::managed::{
    AssetDigest, AssetFile, ManagedRepository, NewResource, NewRevision, PublicationAction,
    PublicationRequest, ResourceKind, RevisionFiles, SnapshotProvenance, SourceSpec,
};
use systemprompt_models::feedback::inventory::{InventoryAvailability, InventoryOrigin};
use systemprompt_models::services::ServicesConfig;
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_db_pool, seed_user_row};

struct Fixture {
    repository: ManagedRepository,
    owner: UserId,
    root: tempfile::TempDir,
}
impl Fixture {
    async fn new() -> Self {
        let bootstrap = ensure_test_bootstrap();
        let db = fixture_db_pool(&bootstrap.database_url)
            .await
            .expect("database");
        let owner = UserId::new(uuid::Uuid::new_v4().to_string());
        seed_user_row(&db, &owner, &format!("{owner}@inventory.invalid"))
            .await
            .expect("owner");
        Self {
            repository: ManagedRepository::new(&db).expect("managed repository"),
            owner,
            root: tempfile::tempdir().expect("services root"),
        }
    }
    fn skill(&self, key: &str, enabled: bool) {
        let path = self.root.path().join("skills").join(key);
        std::fs::create_dir_all(&path).expect("directory");
        std::fs::write(
            path.join("config.yaml"),
            format!(
                "id: {key}\nname: {key}\ndescription: fixture\nenabled: {enabled}\nfile: SKILL.md\n"
            ),
        )
        .expect("config");
        std::fs::write(path.join("SKILL.md"), "# original\n").expect("skill");
    }
    async fn refresh(&self) {
        InventoryService::new(self.repository.clone())
            .refresh(&self.owner, self.root.path(), &ServicesConfig::default())
            .await
            .expect("refresh");
    }
    async fn imported(&self, key: &str) -> (ManagedResourceId, ResourceRevisionId) {
        let source = self
            .repository
            .register_source(
                &self.owner,
                key,
                &SourceSpec::Git {
                    repository: format!("https://example.com/{key}.git"),
                    reference: "main".to_owned(),
                    subdirectory: None,
                    credential_reference: None,
                },
            )
            .await
            .expect("source");
        let snapshot = self
            .repository
            .capture_snapshot(
                &self.owner,
                &source,
                &SnapshotProvenance {
                    source_kind: "git".to_owned(),
                    commit: Some("a".repeat(40)),
                    tree_digest: AssetDigest::of(b"fixture"),
                    importer_version: "fixture".to_owned(),
                },
            )
            .await
            .expect("snapshot");
        let resource = self
            .repository
            .bind_resource(
                &self.owner,
                &NewResource {
                    source_id: source,
                    upstream_key: format!("skills/{key}"),
                    kind: ResourceKind::Skill,
                    resource_key: key.to_owned(),
                },
            )
            .await
            .expect("resource");
        let revision = self
            .repository
            .create_revision(
                &self.owner,
                &NewRevision {
                    resource_id: resource.clone(),
                    snapshot_id: snapshot,
                    parent_id: None,
                    files: RevisionFiles(BTreeMap::from([(
                        "SKILL.md".to_owned(),
                        AssetFile {
                            bytes: b"# imported\n".to_vec(),
                            media_type: "text/markdown".to_owned(),
                            executable: false,
                        },
                    )])),
                    dependencies: BTreeMap::new(),
                    rationale: "fixture import".to_owned(),
                },
            )
            .await
            .expect("revision");
        (resource, revision)
    }
    async fn baselines(&self) -> Vec<systemprompt_marketplace::inventory::BaselineCapture> {
        InventoryService::new(self.repository.clone())
            .prepare_baselines(
                &BaselineScope {
                    owner: &self.owner,
                    actor: &self.owner,
                    root: self.root.path(),
                    services: &ServicesConfig::default(),
                },
                &BaselinePreparation {
                    operation_id: TaskId::generate(),
                    after: None,
                    limit: 100,
                },
            )
            .await
            .expect("baselines")
    }
}


#[path = "publication_admission.rs"]
mod admission;
#[path = "inventory_capture.rs"]
mod capture;
#[path = "inventory_projection.rs"]
mod projection;
#[path = "inventory_publish_latest.rs"]
mod publish_latest;
#[path = "inventory_scanner.rs"]
mod scanner;
