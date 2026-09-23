//! Core Git orchestration resolves registered credentials before source
//! execution.

use systemprompt_identifiers::{ManagedResourceId, ResourceRevisionId, TraceId, UserId};
use systemprompt_marketplace::managed::{GitSyncRequest, ManagedRepository, SourceSpec};
use systemprompt_runtime::managed::git_sources::GitSourceOrchestrator;
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_db_pool, seed_user_row};

#[tokio::test]
async fn missing_private_credentials_fail_initial_import_and_sync_without_launching_git() {
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
        assert!(error.contains("Git credential"));
        assert!(!error.contains(&reference));
    }
}
