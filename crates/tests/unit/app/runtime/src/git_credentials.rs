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
        matches!(error, OrchestrationError::CredentialNotMigrated { .. }),
        "{error}"
    );
    assert!(!error.to_string().contains("xoxb-platform-token"));
}

#[tokio::test]
async fn a_credential_stored_for_one_host_is_not_sent_to_another() {
    let error = sync_with_secrets(
        "https://example.invalid/x.git",
        "ci",
        &[("GIT_SOURCE__GITHUB_2ECOM__CI", "ghp-token")],
    )
    .await;
    assert!(
        matches!(error, OrchestrationError::CredentialUnresolved),
        "{error}"
    );
}

#[tokio::test]
async fn a_credential_under_its_bare_label_fails_naming_the_migration() {
    let error = sync_with_secrets(
        "https://example.invalid/x.git",
        "ci",
        &[("ci", "ghp-token")],
    )
    .await;
    match &error {
        OrchestrationError::CredentialNotMigrated { expected } => {
            assert_eq!(expected, "GIT_SOURCE__EXAMPLE_2EINVALID__CI");
        },
        other => panic!("expected CredentialNotMigrated, got {other}"),
    }
    assert!(
        error
            .to_string()
            .contains("admin config secret migrate-git-credentials"),
        "{error}"
    );
}

#[test]
fn credential_keys_are_namespaced_by_the_repository_host() {
    use systemprompt_runtime::managed::git_sources::git_credential_key;
    assert_eq!(
        git_credential_key("https://GitHub.com/org/repo.git", "ci").as_deref(),
        Some("GIT_SOURCE__GITHUB_2ECOM__CI")
    );
    assert_eq!(
        git_credential_key("https://git.example:8443/repo", "deploy/Key").as_deref(),
        Some("GIT_SOURCE__GIT_2EEXAMPLE_3A8443__DEPLOY_2F_4BEY")
    );
    assert_eq!(git_credential_key("http://github.com/repo", "ci"), None);
    assert_eq!(git_credential_key("https:///repo", "ci"), None);
    assert_eq!(git_credential_key("https://github.com/repo", ""), None);
    assert_ne!(
        git_credential_key("https://a.b/r", "c"),
        git_credential_key("https://a/r", "b.c")
    );
    assert_ne!(
        git_credential_key("https://h/r", "CI"),
        git_credential_key("https://h/r", "ci")
    );
    assert_ne!(
        git_credential_key("https://h/r", "x__y"),
        git_credential_key("https://h__x/r", "y")
    );
}

#[test]
fn credential_keys_use_only_env_safe_characters() {
    use systemprompt_runtime::managed::git_sources::git_credential_key;
    let key = git_credential_key("https://h.example/r", "a-b.c_d/E").expect("key");
    assert!(
        key.bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_'),
        "{key}"
    );
    assert!(key.as_bytes()[0].is_ascii_uppercase());
}

proptest::proptest! {
    #[test]
    fn credential_keys_are_injective_over_host_and_label(
        host_a in "[a-z0-9.:_-]{1,12}",
        label_a in "[A-Za-z0-9._/-]{1,12}",
        host_b in "[a-z0-9.:_-]{1,12}",
        label_b in "[A-Za-z0-9._/-]{1,12}",
    ) {
        use systemprompt_runtime::managed::git_sources::git_credential_key;
        let a = git_credential_key(&format!("https://{host_a}/r"), &label_a).expect("key a");
        let b = git_credential_key(&format!("https://{host_b}/r"), &label_b).expect("key b");
        proptest::prop_assert_eq!(a == b, host_a == host_b && label_a == label_b);
        proptest::prop_assert_eq!(
            a.to_lowercase() == b.to_lowercase(),
            host_a == host_b && label_a == label_b
        );
    }
}
