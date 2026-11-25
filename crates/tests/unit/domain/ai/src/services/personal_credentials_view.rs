// The caller-bound personal credential view reads and removes only the rows
// of the principal it is handed: another user's stored key is invisible to it
// and cannot be removed through it.

use std::sync::Arc;

use systemprompt_ai::PersonalProviderCredentials;
use systemprompt_ai::repository::UserProviderCredentialRepository;
use systemprompt_extension::router::GatewayPrincipal;
use systemprompt_identifiers::CredentialId;
use systemprompt_security::credential::TokenCache;
use systemprompt_test_fixtures::{seed_user_row, test_db_pool, unique_user_id};

#[tokio::test]
async fn the_view_never_reads_or_removes_another_users_credential() {
    let cipher = systemprompt_test_fixtures::fixture_at_rest_cipher();
    let pool = test_db_pool().await;
    let owner = unique_user_id("view-owner");
    let other = unique_user_id("view-other");
    for user in [&owner, &other] {
        seed_user_row(&pool, user, &format!("{}@view.invalid", user.as_str()))
            .await
            .expect("seed user");
    }
    let repo = UserProviderCredentialRepository::new(&pool, cipher.clone());
    let credential = CredentialId::new("opencode-go");
    repo.set_key(
        &owner,
        &credential,
        &cipher.seal("sk-owner").expect("seal"),
        None,
    )
    .await
    .expect("stored");

    let view = PersonalProviderCredentials::new(repo.clone(), Arc::new(TokenCache::default()));
    let as_owner = GatewayPrincipal::__from_kernel(owner.clone());
    let as_other = GatewayPrincipal::__from_kernel(other.clone());

    assert!(view.list(&as_other).await.expect("list").is_empty());
    assert!(!view.remove(&as_other, &credential).await.expect("remove"));
    assert_eq!(view.list(&as_owner).await.expect("list").len(), 1);
    assert_eq!(
        repo.find_key(&owner, &credential)
            .await
            .expect("read")
            .as_deref(),
        Some("sk-owner"),
        "the other principal's remove left the owner's key in place"
    );
    assert!(view.remove(&as_owner, &credential).await.expect("remove"));
    assert!(view.list(&as_owner).await.expect("list").is_empty());
}
