//! Core Git orchestration resolves registered credentials before source
//! execution.

use systemprompt_identifiers::{ManagedResourceId, ResourceRevisionId, TraceId, UserId};
use systemprompt_marketplace::managed::{GitSyncRequest, ManagedRepository, SourceSpec};
use systemprompt_runtime::managed::OrchestrationError;
use systemprompt_runtime::managed::git_sources::GitSourceOrchestrator;
use systemprompt_test_fixtures::{ensure_test_bootstrap, seed_user_row, test_db_pool};

#[tokio::test]
async fn missing_private_credentials_fail_initial_import_and_sync_without_launching_git() {
    ensure_test_bootstrap();
    let db = test_db_pool().await;
    let owner = UserId::new(format!("git-credentials-{}", TraceId::generate()));
    seed_user_row(&db, &owner, &format!("{owner}@credentials.invalid"))
        .await
        .expect("owner");
    let repository = ManagedRepository::new(&db);
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
    let orchestrator = GitSourceOrchestrator::new(
        repository,
        std::env::temp_dir(),
        systemprompt_test_fixtures::ensure_test_secrets_bootstrap(),
    );
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
        .expect_err("unresolved reference");
        assert!(matches!(error, OrchestrationError::CredentialUnresolved));
        assert!(!error.to_string().contains(&reference));
    }
}

#[derive(Debug)]
struct MapSecrets(std::collections::BTreeMap<String, String>);

#[async_trait::async_trait]
impl systemprompt_traits::SecretStore for MapSecrets {
    fn secret(&self, name: &str) -> Option<systemprompt_traits::ExtensionSecret> {
        self.0
            .get(name)
            .cloned()
            .map(systemprompt_traits::ExtensionSecret::new)
    }

    fn key_names(&self) -> Result<Vec<String>, systemprompt_traits::SecretStoreError> {
        Ok(self.0.keys().cloned().collect())
    }

    fn manifest_signing_secret_seed(
        &self,
    ) -> Result<[u8; systemprompt_traits::SECRET_SEED_BYTES], systemprompt_traits::SecretStoreError>
    {
        Ok([0; systemprompt_traits::SECRET_SEED_BYTES])
    }

    fn encryption_master_key(
        &self,
    ) -> Result<[u8; systemprompt_traits::SECRET_SEED_BYTES], systemprompt_traits::SecretStoreError>
    {
        Ok([0; systemprompt_traits::SECRET_SEED_BYTES])
    }

    fn status(&self) -> systemprompt_traits::SecretsStatus {
        systemprompt_traits::SecretsStatus {
            source: "test".to_owned(),
            generation: 0,
            initialised_at: chrono::Utc::now(),
            last_reload_at: None,
            last_error: None,
        }
    }

    fn refresh_interval(&self) -> Option<std::time::Duration> {
        None
    }

    async fn reload(
        &self,
    ) -> Result<systemprompt_traits::ReloadOutcome, systemprompt_traits::SecretStoreError> {
        Ok(systemprompt_traits::ReloadOutcome {
            generation: 0,
            changed_keys: Vec::new(),
            swapped: false,
        })
    }
}

async fn sync_with_secrets(
    repository_url: &str,
    reference: &str,
    secrets: &[(&str, &str)],
) -> OrchestrationError {
    ensure_test_bootstrap();
    let db = test_db_pool().await;
    let owner = UserId::new(format!("git-credentials-{}", TraceId::generate()));
    seed_user_row(&db, &owner, &format!("{owner}@credentials.invalid"))
        .await
        .expect("owner");
    let repository = ManagedRepository::new(&db);
    let source = repository
        .register_source(
            &owner,
            "private",
            &SourceSpec::Git {
                repository: repository_url.to_owned(),
                reference: "main".to_owned(),
                subdirectory: None,
                credential_reference: Some(reference.to_owned()),
            },
        )
        .await
        .expect("registered private source");
    let store = MapSecrets(
        secrets
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect(),
    );
    let orchestrator =
        GitSourceOrchestrator::new(repository, std::env::temp_dir(), std::sync::Arc::new(store));
    let request = GitSyncRequest {
        source_id: source,
        resource_id: ManagedResourceId::generate(),
        upstream_root: "skill".to_owned(),
        upstream_base_revision_id: None,
    };
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        orchestrator.synchronize(&owner, &request),
    )
    .await
    .expect("credential rejection before git runs")
    .expect_err("sync refused")
}

#[tokio::test]
async fn a_reference_cannot_name_a_secret_outside_the_git_source_namespace() {
    let error = sync_with_secrets(
        "https://example.invalid/x.git",
        "slack_bot_token",
        &[("slack_bot_token", "xoxb-platform-token")],
    )
    .await;
    assert!(
        matches!(error, OrchestrationError::CredentialUnresolved),
        "{error}"
    );
}

#[tokio::test]
async fn a_credential_stored_for_one_host_is_not_sent_to_another() {
    let error = sync_with_secrets(
        "https://example.invalid/x.git",
        "ci",
        &[
            ("git_source/github.com/ci", "ghp-token"),
            ("ci", "ghp-token"),
        ],
    )
    .await;
    assert!(
        matches!(error, OrchestrationError::CredentialUnresolved),
        "{error}"
    );
}

#[test]
fn credential_keys_are_namespaced_by_the_repository_host() {
    use systemprompt_runtime::managed::git_sources::git_credential_key;
    assert_eq!(
        git_credential_key("https://GitHub.com/org/repo.git", "ci").as_deref(),
        Some("git_source/github.com/ci")
    );
    assert_eq!(
        git_credential_key("https://git.example:8443/repo", "deploy/key").as_deref(),
        Some("git_source/git.example:8443/deploy/key")
    );
    assert_eq!(git_credential_key("http://github.com/repo", "ci"), None);
    assert_eq!(git_credential_key("https:///repo", "ci"), None);
    assert_eq!(git_credential_key("https://github.com/repo", ""), None);
}
