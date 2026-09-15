use chrono::Utc;
use sqlx::PgPool;
use std::collections::BTreeMap;
use systemprompt_identifiers::{
    ConsumerInstallationId, DeviceCertId, NativeSessionId, ResourceInvocationId, UserId,
};
use systemprompt_marketplace::managed::consumer::{
    ConsumerInvocationRequest, IssuedConsumerCredential,
};
use systemprompt_marketplace::managed::{
    AssetDigest, AssetFile, ManagedRepository, NewResource, NewRevision, PublicationAction,
    PublicationRequest, ResourceKind, RevisionFiles, SnapshotProvenance, SourceSpec,
};
use systemprompt_models::feedback::receipts::{
    ConsumerReceiptRequest, FileReadback, ReadbackStatus, SessionBindingRequest,
};
use systemprompt_models::feedback::{ContentDigest, EvaluatorClient};
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_db_pool, seed_user_row};

pub struct Fixture {
    pub repo: ManagedRepository,
    pub pool: PgPool,
    pub owner: UserId,
    pub consumer: UserId,
    pub cert: DeviceCertId,
    pub credential: IssuedConsumerCredential,
    pub request: ConsumerReceiptRequest,
}

pub async fn fixture() -> Fixture {
    fixture_with_metadata(None).await
}

pub async fn fixture_with_metadata(metadata: Option<(&str, &str)>) -> Fixture {
    let bootstrap = ensure_test_bootstrap();
    let db = fixture_db_pool(&bootstrap.database_url)
        .await
        .expect("database required");
    let pool = db.write_pool_arc().expect("write pool").as_ref().clone();
    let owner = UserId::new(uuid::Uuid::new_v4().to_string());
    let consumer = UserId::new(uuid::Uuid::new_v4().to_string());
    seed_user_row(&db, &owner, &format!("{owner}@consumer-test.invalid"))
        .await
        .expect("publisher");
    seed_user_row(&db, &consumer, &format!("{consumer}@consumer-test.invalid"))
        .await
        .expect("consumer");
    let cert = DeviceCertId::generate();
    sqlx::query!("INSERT INTO user_device_certs(id,user_id,fingerprint,label) VALUES($1,$2,$3,'consumer test')", cert.as_str(), consumer.as_str(), cert.as_str()).execute(&pool).await.expect("enrolled device");
    let repo = ManagedRepository::new(&db).expect("managed repository");
    let source = repo
        .register_source(&owner, "authoring", &SourceSpec::Managed)
        .await
        .expect("source");
    let snapshot = repo
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
    let resource = repo
        .bind_resource(
            &owner,
            &NewResource {
                source_id: source,
                upstream_key: "skill".to_owned(),
                kind: ResourceKind::Skill,
                resource_key: "skill".to_owned(),
            },
        )
        .await
        .expect("resource");
    let mut files = RevisionFiles(BTreeMap::from([
        (
            "SKILL.md".to_owned(),
            AssetFile {
                bytes: b"# Skill".to_vec(),
                media_type: "text/markdown".to_owned(),
                executable: false,
            },
        ),
        (
            "run.sh".to_owned(),
            AssetFile {
                bytes: b"true".to_vec(),
                media_type: "text/plain".to_owned(),
                executable: true,
            },
        ),
    ]));
    if let Some((name, description)) = metadata {
        files.0.insert("config.yaml".to_owned(), AssetFile {
            bytes: serde_json::to_vec(&serde_json::json!({"id":"skill","name":name,"description":description,"file":"SKILL.md"})).unwrap(),
            media_type: "application/yaml".to_owned(),
            executable: false,
        });
    }
    let revision = repo
        .create_revision(
            &owner,
            &NewRevision {
                resource_id: resource.clone(),
                snapshot_id: snapshot,
                parent_id: None,
                files,
                dependencies: BTreeMap::new(),
                rationale: "test".to_owned(),
            },
        )
        .await
        .expect("revision");
    let publication = repo
        .review_and_publish(
            &owner,
            &owner,
            &PublicationRequest {
                resource_id: resource.clone(),
                revision_id: Some(revision.clone()),
                action: PublicationAction::InitialAdoption,
                expected_generation: 0,
                operation_key: "initial".to_owned(),
                comparison_evidence: systemprompt_marketplace::managed::ComparisonEvidence::default(
                ),
                limitations: String::new(),
            },
        )
        .await
        .expect("publication");
    let bundle = repo
        .get_revision_bundle(&owner, &revision)
        .await
        .expect("bundle");
    let files = bundle
        .revisions
        .iter()
        .flat_map(|(revision_id, manifest)| {
            manifest
                .files
                .iter()
                .map(move |(path, entry)| FileReadback {
                    revision_id: revision_id.clone(),
                    path: path.clone(),
                    digest: ContentDigest::try_from(entry.digest.as_str().to_owned()).unwrap(),
                    bytes: entry.bytes,
                    executable: entry.executable,
                    content_check: ReadbackStatus::Verified,
                    mode_check: ReadbackStatus::Verified,
                })
        })
        .collect();
    let mut request = ConsumerReceiptRequest {
        installation_id: ConsumerInstallationId::generate(),
        publication_id: publication.publication_id,
        resource_id: resource,
        revision_id: revision,
        generation: 1,
        bundle_digest: ContentDigest::try_from(bundle.digest().unwrap().as_str().to_owned())
            .unwrap(),
        host: EvaluatorClient::Codex,
        observed_at: Utc::now(),
        files,
        runtime_files: Vec::new(),
    };
    let credential = repo
        .issue_consumer_credential(&cert)
        .await
        .expect("credential");
    repo.set_consumer_grant(&owner, &request.resource_id, &consumer, true)
        .await
        .expect("grant");
    let plan = repo
        .consumer_installation_plan(
            &credential.credential,
            &request.resource_id,
            &request.publication_id,
            request.host,
        )
        .await
        .expect("installation plan");
    request.runtime_files = plan
        .runtime_files
        .iter()
        .map(
            |file| systemprompt_models::feedback::receipts::RuntimeFileReadback {
                path: file.path.clone(),
                digest: ContentDigest::of(&file.bytes),
                bytes: file.bytes.len() as u64,
                executable: file.executable,
                content_check: ReadbackStatus::Verified,
                mode_check: ReadbackStatus::Verified,
            },
        )
        .collect();
    Fixture {
        repo,
        pool,
        owner,
        consumer,
        cert,
        credential,
        request,
    }
}

impl Fixture {
    pub fn invocation(&self) -> ConsumerInvocationRequest {
        ConsumerInvocationRequest {
            invocation_id: ResourceInvocationId::new("invocation"),
            host: self.request.host,
            session_id: NativeSessionId::new("session"),
            resource_id: self.request.resource_id.clone(),
            installation_id: Some(self.request.installation_id.clone()),
            revision_id: Some(self.request.revision_id.clone()),
            generation: Some(self.request.generation),
            occurred_at: Utc::now(),
            evidence: serde_json::json!({"source":"authenticated-bridge"}),
        }
    }

    pub async fn receipt_binding(&self) -> SessionBindingRequest {
        let receipt = self
            .repo
            .record_consumer_receipt(&self.credential.credential, &self.request)
            .await
            .expect("receipt");
        SessionBindingRequest {
            receipt_id: receipt.receipt_id,
            host: self.request.host,
            session_id: NativeSessionId::new("session"),
        }
    }

    pub async fn grant(&self, enabled: bool) {
        self.repo
            .set_consumer_grant(
                &self.owner,
                &self.request.resource_id,
                &self.consumer,
                enabled,
            )
            .await
            .expect("grant change");
    }
}
