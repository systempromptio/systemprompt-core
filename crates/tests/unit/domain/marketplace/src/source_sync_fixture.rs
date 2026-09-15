//! Trusted-capture fixture; native authenticated HTTPS is covered by git_https.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use systemprompt_identifiers::UserId;
use systemprompt_marketplace::managed::{
    AssetFile, CapturedGitSource, GitCaptureRequest, GitSourceCapture, GitSyncRequest,
    GitSyncResult, GitSynchronizationService, ManagedError, ManagedRepository, NewResource,
    PublicationAction, PublicationRequest, ResourceKind, Result, RevisionFiles, SourceSpec,
};
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_db_pool, seed_user_row};

pub(super) struct Capture {
    pub output: Mutex<(String, RevisionFiles)>,
    pub calls: AtomicUsize,
}
impl GitSourceCapture for Capture {
    fn capture(&self, request: &GitCaptureRequest<'_>) -> Result<CapturedGitSource> {
        let GitCaptureRequest {
            repository,
            reference,
            subdirectory,
            root,
            credential,
        } = *request;
        self.calls.fetch_add(1, Ordering::SeqCst);
        assert_eq!(repository, "https://git.example.com/organization.git");
        assert_eq!(reference, "refs/heads/main");
        assert_eq!(subdirectory, Some("catalog"));
        assert_eq!(root, "alpha");
        if credential != Some("scoped-source-secret") {
            return Err(ManagedError::Unavailable);
        }
        let (commit, files) = self.output.lock().unwrap().clone();
        Ok(CapturedGitSource { commit, files })
    }
}

pub(super) struct Fixture {
    pub repo: ManagedRepository,
    pub owner: UserId,
    pub request: GitSyncRequest,
    pub capture: Arc<Capture>,
    pub service: GitSynchronizationService,
}
impl Fixture {
    pub async fn new() -> Self {
        let bootstrap = ensure_test_bootstrap();
        let db = fixture_db_pool(&bootstrap.database_url)
            .await
            .expect("source sync database required");
        let owner = UserId::new(uuid::Uuid::new_v4().to_string());
        seed_user_row(&db, &owner, &format!("{owner}@source-sync.invalid"))
            .await
            .unwrap();
        let repo = ManagedRepository::new(&db).expect("managed repository");
        let source = repo
            .register_source(
                &owner,
                "source-sync",
                &SourceSpec::Git {
                    repository: "https://git.example.com/organization.git".into(),
                    reference: "refs/heads/main".into(),
                    subdirectory: Some("catalog".into()),
                    credential_reference: Some("organization-git-secret".into()),
                },
            )
            .await
            .unwrap();
        let resource = repo
            .bind_resource(
                &owner,
                &NewResource {
                    source_id: source.clone(),
                    upstream_key: "alpha".into(),
                    kind: ResourceKind::Skill,
                    resource_key: "alpha".into(),
                },
            )
            .await
            .unwrap();
        let capture = Arc::new(Capture {
            output: Mutex::new(("a".repeat(40), files("base"))),
            calls: AtomicUsize::new(0),
        });
        let service = GitSynchronizationService::new(repo.clone(), capture.clone());
        Self {
            repo,
            owner,
            request: GitSyncRequest {
                source_id: source,
                resource_id: resource,
                upstream_root: "alpha".into(),
                upstream_base_revision_id: None,
            },
            capture,
            service,
        }
    }
    pub async fn sync(&self) -> Result<GitSyncResult> {
        self.service
            .sync(&self.owner, &self.request, Some("scoped-source-secret"))
            .await
    }
    pub fn output(&self, commit: char, files: RevisionFiles) {
        *self.capture.output.lock().unwrap() = (commit.to_string().repeat(40), files);
    }
    pub async fn publish(&self, revision: systemprompt_identifiers::ResourceRevisionId) {
        self.repo
            .review_and_publish(
                &self.owner,
                &self.owner,
                &PublicationRequest {
                    resource_id: self.request.resource_id.clone(),
                    revision_id: Some(revision),
                    action: PublicationAction::InitialAdoption,
                    expected_generation: 0,
                    operation_key: "initial-source-sync-publication".into(),
                    comparison_evidence:
                        systemprompt_marketplace::managed::ComparisonEvidence::default(),
                    limitations: String::new(),
                },
            )
            .await
            .unwrap();
    }
}

pub(super) fn files(body: &str) -> RevisionFiles {
    RevisionFiles(BTreeMap::from([
        (
            "config.yaml".into(),
            AssetFile {
                bytes: b"id: alpha\nenabled: true\n".to_vec(),
                media_type: "application/yaml".into(),
                executable: false,
            },
        ),
        (
            "index.md".into(),
            AssetFile {
                bytes: body.as_bytes().to_vec(),
                media_type: "text/markdown".into(),
                executable: false,
            },
        ),
    ]))
}
