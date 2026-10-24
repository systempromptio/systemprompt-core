// `user_provider_credentials` is keyed by (user, catalog credential id): one
// stored key per member per credential, sealed at rest, opened only by
// `find_key`.

use systemprompt_ai::repository::UserProviderCredentialRepository;
use systemprompt_identifiers::CredentialId;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, ensure_test_secrets_bootstrap, seed_user_row, test_db_pool,
    unique_user_id,
};

#[tokio::test]
async fn a_key_is_stored_found_listed_and_deleted_per_credential() {
    ensure_test_bootstrap();
    ensure_test_secrets_bootstrap();
    let pool = test_db_pool().await;
    let user = unique_user_id("personal-cred");
    seed_user_row(&pool, &user, &format!("{}@personal.invalid", user.as_str()))
        .await
        .expect("seed user");
    let repo = UserProviderCredentialRepository::new(&pool);
    let go = CredentialId::new("opencode-go");
    let other = CredentialId::new("other-sub");

    let sealed = systemprompt_security::at_rest::seal("sk-go-1").expect("seal");
    let meta = repo
        .set_key(&user, &go, &sealed, Some("Go"))
        .await
        .expect("stored");
    assert_eq!(meta.credential, "opencode-go");
    assert_eq!(meta.label.as_deref(), Some("Go"));

    let replaced = systemprompt_security::at_rest::seal("sk-go-2").expect("seal");
    repo.set_key(&user, &go, &replaced, None)
        .await
        .expect("one key per member and credential: the second store replaces the first");
    assert_eq!(
        repo.find_key(&user, &go).await.expect("read").as_deref(),
        Some("sk-go-2")
    );
    assert_eq!(repo.find_key(&user, &other).await.expect("read"), None);

    let listed = repo.list_metadata(&user).await.expect("list");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].credential, "opencode-go");
    assert!(listed[0].last_used_at.is_some(), "find_key stamps last use");

    assert!(repo.delete_key(&user, &go).await.expect("delete"));
    assert!(!repo.delete_key(&user, &go).await.expect("delete twice"));
}
