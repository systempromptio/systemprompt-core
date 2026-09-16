//! Core Git orchestration resolves registered credentials before source
//! execution.

use systemprompt_identifiers::{ManagedResourceId, ResourceRevisionId, TraceId, UserId};
use systemprompt_marketplace::managed::{GitSyncRequest, ManagedRepository, SourceSpec};
use systemprompt_models::feedback::verification::{
    DependencyVerificationInput, DependencyVerificationRequest,
};
use systemprompt_runtime::optimization::git_sources::GitSourceOrchestrator;
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_db_pool, seed_user_row};

#[tokio::test]
async fn missing_private_credentials_fail_identically_for_initial_import_sync_and_verification() {
    let bootstrap = ensure_test_bootstrap();
    let db = fixture_db_pool(&bootstrap.database_url)
        .await
        .expect("database");
    let owner = UserId::new(format!("git-credentials-{}", TraceId::generate()));
    seed_user_row(&db, &owner, &format!("{owner}@credentials.invalid"))
        .await
        .expect("owner");
    let repository = ManagedRepository::new(&db).expect("managed repository");
    let reference = format!("absent-private-reference-{}", TraceId::generate());
    let source = repository
        .register_source(
            &owner,
            "private",
            &SourceSpec::Git {
                repository: "https://example.invalid/unreachable.git".to_owned(),
                reference: "main".to_owned(),
                subdirectory: None,
                credential_reference: Some(reference.clone()),
            },
        )
        .await
        .expect("registered private source");
    let orchestrator = GitSourceOrchestrator::new(repository);
    let revision = ResourceRevisionId::generate();
    let verification = DependencyVerificationRequest {
        root_revision_id: revision.clone(),
        revisions: vec![DependencyVerificationInput {
            revision_id: revision.clone(),
            source_id: source.clone(),
            exact_commit: "a".repeat(40),
            relative_root: "skill".to_owned(),
            dependencies: Vec::new(),
        }],
    };
    let verification_error = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        orchestrator.verify(&owner, &verification),
    )
    .await
    .expect("credential rejection does not launch Git")
    .expect_err("unresolved reference")
    .to_string();
    assert!(verification_error.contains("Git credential"));
    assert!(!verification_error.contains(&reference));
    for base in [None, Some(revision.clone())] {
        let request = GitSyncRequest {
            source_id: source.clone(),
            resource_id: ManagedResourceId::generate(),
            upstream_root: "skill".to_owned(),
            upstream_base_revision_id: base,
        };
        let error = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            orchestrator.synchronize(&owner, &request),
        )
        .await
        .expect("credential rejection before import/sync")
        .expect_err("unresolved reference")
        .to_string();
        assert_eq!(error, verification_error);
    }
}

#[tokio::test]
async fn verification_resolves_each_dependency_source_and_rejects_non_git_registration() {
    let bootstrap = ensure_test_bootstrap();
    let db = fixture_db_pool(&bootstrap.database_url)
        .await
        .expect("database");
    let owner = UserId::new(format!(
        "git-dependency-credentials-{}",
        TraceId::generate()
    ));
    seed_user_row(&db, &owner, &format!("{owner}@credentials.invalid"))
        .await
        .expect("owner");
    let repository = ManagedRepository::new(&db).expect("managed repository");
    let public = repository
        .register_source(
            &owner,
            "public-root",
            &SourceSpec::Git {
                repository: "https://example.invalid/root.git".to_owned(),
                reference: "main".to_owned(),
                subdirectory: None,
                credential_reference: None,
            },
        )
        .await
        .unwrap();
    let private = repository
        .register_source(
            &owner,
            "private-dependency",
            &SourceSpec::Git {
                repository: "https://example.invalid/dependency.git".to_owned(),
                reference: "main".to_owned(),
                subdirectory: None,
                credential_reference: Some(format!("absent-{}", TraceId::generate())),
            },
        )
        .await
        .unwrap();
    let local = repository
        .register_source(
            &owner,
            "local",
            &SourceSpec::LocalTree {
                root: "/configured/local".to_owned(),
            },
        )
        .await
        .unwrap();
    let root = ResourceRevisionId::generate();
    let dependency = ResourceRevisionId::generate();
    let mut request = DependencyVerificationRequest {
        root_revision_id: root.clone(),
        revisions: vec![
            DependencyVerificationInput {
                revision_id: root,
                source_id: public,
                exact_commit: "a".repeat(40),
                relative_root: "root".to_owned(),
                dependencies: vec![dependency.clone()],
            },
            DependencyVerificationInput {
                revision_id: dependency,
                source_id: private,
                exact_commit: "b".repeat(40),
                relative_root: "dependency".to_owned(),
                dependencies: Vec::new(),
            },
        ],
    };
    let orchestrator = GitSourceOrchestrator::new(repository);
    let error = orchestrator
        .verify(&owner, &request)
        .await
        .expect_err("dependency independently requires credentials");
    assert!(error.to_string().contains("Git credential"));
    request.revisions[1].source_id = local;
    let error = orchestrator
        .verify(&owner, &request)
        .await
        .expect_err("local source cannot supply Git provenance");
    assert!(error.to_string().contains("registered Git source"));
}
