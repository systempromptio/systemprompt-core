use std::collections::BTreeMap;
use std::sync::Arc;
use systemprompt_identifiers::{ManagedSourceId, ResourceRevisionId, UserId};
use systemprompt_marketplace::managed::{
    AssetDigest, AssetFile, DependencyRef, GitSourceBinding, GitTreeRead, GitTreeReader,
    GitVerificationService, ManagedError, ManagedRepository, NewResource, NewRevision,
    ResourceKind, RevisionFiles, SnapshotProvenance, SourceSpec,
};
use systemprompt_models::feedback::verification::{
    DependencyVerificationInput, DependencyVerificationRequest,
};
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_db_pool, seed_user_row};

struct Reader {
    credentials: BTreeMap<ManagedSourceId, String>,
    files: BTreeMap<ResourceRevisionId, RevisionFiles>,
    corrupt_mode: bool,
    corrupt_bytes: bool,
}

impl GitTreeReader for Reader {
    fn read(&self, request: &GitTreeRead<'_>) -> Result<RevisionFiles, ManagedError> {
        let GitTreeRead {
            input,
            repository,
            credential,
            deadline,
            ..
        } = *request;
        assert!(repository.starts_with("https://git.example.com/"));
        assert!(deadline > std::time::Instant::now());
        if credential != self.credentials.get(&input.source_id).map(String::as_str) {
            return Err(ManagedError::Unavailable);
        }
        let mut files = self
            .files
            .get(&input.revision_id)
            .expect("registered fixture revision")
            .clone();
        for file in files.0.values_mut() {
            if self.corrupt_mode {
                file.executable = !file.executable;
            }
            if self.corrupt_bytes {
                file.bytes.push(0);
            }
        }
        Ok(files)
    }
}

struct Fixture {
    repository: ManagedRepository,
    owner: UserId,
    request: DependencyVerificationRequest,
    credentials: BTreeMap<ManagedSourceId, String>,
    files: BTreeMap<ResourceRevisionId, RevisionFiles>,
}

impl Fixture {
    async fn new() -> Self {
        Self::with_authored_root(false).await
    }

    async fn with_authored_root(authored_root: bool) -> Self {
        let bootstrap = ensure_test_bootstrap();
        let db = fixture_db_pool(&bootstrap.database_url)
            .await
            .expect("test database");
        let owner = UserId::new(uuid::Uuid::new_v4().to_string());
        seed_user_row(&db, &owner, &format!("{}@git.invalid", owner.as_str()))
            .await
            .expect("owner");
        let repository = ManagedRepository::new(&db).expect("managed repository");
        let mut credentials = BTreeMap::new();
        let mut files_by_revision = BTreeMap::new();
        let mut inputs = Vec::new();
        let mut dependencies = BTreeMap::new();
        for name in ["dependency", "root"] {
            let source = repository
                .register_source(
                    &owner,
                    name,
                    &SourceSpec::Git {
                        repository: format!("https://git.example.com/{name}.git"),
                        reference: "main".to_owned(),
                        subdirectory: None,
                        credential_reference: Some(format!("git-{name}")),
                    },
                )
                .await
                .expect("source");
            credentials.insert(source.clone(), format!("secret-{name}"));
            let files = RevisionFiles(BTreeMap::from([(
                "run.sh".to_owned(),
                AssetFile {
                    bytes: b"#!/bin/sh\nexit 0\n".to_vec(),
                    executable: true,
                    media_type: "text/plain".to_owned(),
                },
            )]));
            let authored = authored_root && name == "root";
            let content_source = if authored {
                repository
                    .register_source(
                        &owner,
                        "local-authoring",
                        &SourceSpec::LocalTree {
                            root: "/configured/authoring".to_owned(),
                        },
                    )
                    .await
                    .expect("local source")
            } else {
                source.clone()
            };
            let snapshot = repository
                .capture_snapshot(
                    &owner,
                    &content_source,
                    &SnapshotProvenance {
                        source_kind: if authored { "local_tree" } else { "git" }.to_owned(),
                        commit: if authored { None } else { Some("a".repeat(40)) },
                        tree_digest: AssetDigest::of(b"fixture-tree"),
                        importer_version: "fixture".to_owned(),
                    },
                )
                .await
                .expect("snapshot");
            let resource = repository
                .bind_resource(
                    &owner,
                    &NewResource {
                        source_id: content_source,
                        upstream_key: name.to_owned(),
                        kind: ResourceKind::Supporting,
                        resource_key: name.to_owned(),
                    },
                )
                .await
                .expect("resource");
            let dependency_ids = dependencies
                .values()
                .map(|value: &DependencyRef| value.revision_id.clone())
                .collect();
            let revision = repository
                .create_revision(
                    &owner,
                    &NewRevision {
                        resource_id: resource,
                        snapshot_id: snapshot,
                        parent_id: None,
                        files: files.clone(),
                        dependencies: dependencies.clone(),
                        rationale: "fixture exact Git revision".to_owned(),
                    },
                )
                .await
                .expect("revision");
            files_by_revision.insert(revision.clone(), files);
            inputs.push(DependencyVerificationInput {
                revision_id: revision.clone(),
                source_id: source,
                exact_commit: "a".repeat(40),
                relative_root: name.to_owned(),
                dependencies: dependency_ids,
            });
            dependencies.insert(
                name.to_owned(),
                DependencyRef {
                    revision_id: revision.clone(),
                    digest: repository
                        .get_revision(&owner, &revision)
                        .await
                        .expect("retained revision")
                        .digest()
                        .expect("digest"),
                },
            );
        }
        let request = DependencyVerificationRequest {
            root_revision_id: inputs.last().expect("root").revision_id.clone(),
            revisions: inputs,
        };
        Self {
            repository,
            owner,
            request,
            credentials,
            files: files_by_revision,
        }
    }

    fn service(&self, corrupt_mode: bool, corrupt_bytes: bool) -> GitVerificationService {
        GitVerificationService::new(
            self.repository.clone(),
            Arc::new(Reader {
                credentials: self.credentials.clone(),
                files: self.files.clone(),
                corrupt_mode,
                corrupt_bytes,
            }),
        )
    }
}

#[tokio::test]
async fn complete_dependency_manifest_retains_independent_source_evidence_and_identical_retry() {
    let f = Fixture::new().await;
    let service = f.service(false, false);
    let first = service
        .verify(&f.owner, &f.request, &f.credentials)
        .await
        .expect("complete verification");
    assert_eq!(first.revisions.len(), 2);
    first.validate_complete().expect("complete evidence");
    let retry = service
        .verify(&f.owner, &f.request, &f.credentials)
        .await
        .expect("identical retry");
    assert_eq!(retry.id, first.id);
    f.repository
        .require_verified_git_content(&f.owner, &f.request.root_revision_id, &"a".repeat(40))
        .await
        .expect("attestable complete manifest");
}

#[tokio::test]
async fn rejects_wrong_source_commit_root_credentials_and_incomplete_dependencies() {
    let f = Fixture::new().await;
    let service = f.service(false, false);
    for changed in 0..5 {
        let mut request = f.request.clone();
        let mut credentials = f.credentials.clone();
        match changed {
            0 => request.revisions[0].exact_commit = "b".repeat(40),
            1 => request.revisions[0].relative_root = "unregistered".to_owned(),
            2 => request.revisions[0].source_id = request.revisions[1].source_id.clone(),
            3 => {
                credentials.remove(&request.revisions[0].source_id);
            },
            _ => {
                request.revisions.remove(0);
            },
        }
        assert!(
            service
                .verify(&f.owner, &request, &credentials)
                .await
                .is_err()
        );
    }
    assert!(
        f.repository
            .require_verified_git_content(&f.owner, &f.request.root_revision_id, &"a".repeat(40))
            .await
            .is_err()
    );
}

#[tokio::test]
async fn byte_and_executable_mode_mismatch_never_attest() {
    let f = Fixture::new().await;
    for (mode, bytes) in [(true, false), (false, true)] {
        assert!(
            f.service(mode, bytes)
                .verify(&f.owner, &f.request, &f.credentials)
                .await
                .is_err()
        );
    }
    assert!(
        f.repository
            .require_verified_git_content(&f.owner, &f.request.root_revision_id, &"a".repeat(40))
            .await
            .is_err()
    );
}

#[tokio::test]
async fn local_authored_root_requires_retained_source_binding_then_verifies_committed_content() {
    let f = Fixture::with_authored_root(true).await;
    let service = f.service(false, false);
    assert!(
        service
            .verify(&f.owner, &f.request, &f.credentials)
            .await
            .is_err()
    );
    let root = f.request.revisions.last().expect("root");
    let resource = f
        .repository
        .revision_resource(&f.owner, &root.revision_id)
        .await
        .expect("root resource");
    f.repository
        .bind_git_verification_source(
            &f.owner,
            &f.owner,
            &GitSourceBinding {
                resource: &resource,
                source: &root.source_id,
                relative_root: &root.relative_root,
            },
        )
        .await
        .expect("explicit source binding");
    assert!(
        f.repository
            .bind_git_verification_source(
                &f.owner,
                &f.owner,
                &GitSourceBinding {
                    resource: &resource,
                    source: &root.source_id,
                    relative_root: "conflicting-root",
                },
            )
            .await
            .is_err()
    );
    service
        .verify(&f.owner, &f.request, &f.credentials)
        .await
        .expect("authored root committed bytes verified");
}

#[tokio::test]
async fn cyclic_or_undeclared_dependency_graph_cannot_retain_attestation() {
    let f = Fixture::new().await;
    let service = f.service(false, false);
    let mut cyclic = f.request.clone();
    cyclic.revisions[0]
        .dependencies
        .push(cyclic.root_revision_id.clone());
    assert!(
        service
            .verify(&f.owner, &cyclic, &f.credentials)
            .await
            .is_err()
    );
    let mut undeclared = f.request.clone();
    // Still a valid connected DAG, but reverses the immutable retained dependency
    // edge.
    undeclared.root_revision_id = undeclared.revisions[0].revision_id.clone();
    let previous_root = undeclared.revisions[1].revision_id.clone();
    undeclared.revisions[0].dependencies.push(previous_root);
    undeclared.revisions[1].dependencies.clear();
    assert!(
        service
            .verify(&f.owner, &undeclared, &f.credentials)
            .await
            .is_err()
    );
    assert!(
        f.repository
            .require_verified_git_content(&f.owner, &f.request.root_revision_id, &"a".repeat(40))
            .await
            .is_err()
    );
    service
        .verify(&f.owner, &f.request, &f.credentials)
        .await
        .expect("invalid graph attempts did not poison later complete verification");
}

#[tokio::test]
async fn dependency_credentials_cannot_be_swapped_or_reused_after_independent_rotation() {
    let mut f = Fixture::new().await;
    let source = f.request.revisions[0].source_id.clone();
    let other_source = f.request.revisions[1].source_id.clone();
    let mut swapped = f.credentials.clone();
    swapped.insert(source.clone(), f.credentials[&other_source].clone());
    assert!(
        f.service(false, false)
            .verify(&f.owner, &f.request, &swapped)
            .await
            .is_err()
    );
    assert!(
        f.repository
            .require_verified_git_content(&f.owner, &f.request.root_revision_id, &"a".repeat(40))
            .await
            .is_err()
    );
    let before_rotation = f.credentials.clone();
    f.credentials
        .insert(source, "rotated-dependency-only".to_owned());
    let service = f.service(false, false);
    assert!(
        service
            .verify(&f.owner, &f.request, &before_rotation)
            .await
            .is_err()
    );
    let manifest = service
        .verify(&f.owner, &f.request, &f.credentials)
        .await
        .expect("independent dependency rotation leaves root credential valid");
    manifest.validate_complete().expect("complete verification");
    let evidence = serde_json::to_string(&manifest).expect("serializable evidence");
    for secret in f.credentials.values().chain(before_rotation.values()) {
        assert!(
            !evidence.contains(secret),
            "retained manifest must not contain credentials"
        );
    }
}


#[test]
fn git_request_debug_redacts_credentials_in_normal_and_pretty_output() {
    use systemprompt_marketplace::managed::GitCaptureRequest;
    let credential = "fixture-private-git-token-not-for-logs";
    let input = DependencyVerificationInput {
        revision_id: ResourceRevisionId::new("revision"),
        source_id: ManagedSourceId::new("source"),
        exact_commit: "a".repeat(40),
        relative_root: "skill".to_owned(),
        dependencies: Vec::new(),
    };
    let capture = GitCaptureRequest {
        repository: "https://example.invalid/repo.git",
        reference: "refs/heads/next",
        subdirectory: None,
        root: "skill",
        credential: Some(credential),
    };
    let read = GitTreeRead {
        input: &input,
        repository: capture.repository,
        subdirectory: None,
        credential: Some(credential),
        deadline: std::time::Instant::now(),
    };
    for output in [
        format!("{capture:?}"),
        format!("{capture:#?}"),
        format!("{read:?}"),
        format!("{read:#?}"),
    ] {
        assert!(!output.contains(credential));
        assert!(output.contains("<redacted>"));
        assert!(output.contains("https://example.invalid/repo.git"));
    }
    assert_eq!(capture.credential, Some(credential));
    assert_eq!(read.credential, Some(credential));
    let anonymous = GitCaptureRequest {
        credential: None,
        ..capture
    };
    assert!(format!("{anonymous:?}").contains("credential: None"));
    let public_read = GitTreeRead {
        credential: None,
        ..read
    };
    assert!(format!("{public_read:?}").contains("credential: None"));
}
