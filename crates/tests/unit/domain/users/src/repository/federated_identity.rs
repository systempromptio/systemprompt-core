//! DB-backed tests for the federated-identity repository
//! (`find_federated`, `find_or_create_federated`).

use systemprompt_identifiers::UserId;
use systemprompt_test_fixtures::{ensure_test_bootstrap, test_db_pool};
use systemprompt_traits::FederatedIdentityClaims;
use systemprompt_users::UserRepository;
use uuid::Uuid;

struct Ctx {
    repo: UserRepository,
    issuer: String,
    external_sub: String,
}

async fn setup(prefix: &str) -> Ctx {
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let repo = UserRepository::new(&pool).expect("repo");
    let tag = Uuid::new_v4();
    Ctx {
        repo,
        issuer: format!("https://idp-{prefix}-{tag}.example.com/realm"),
        external_sub: format!("sub-{prefix}-{tag}"),
    }
}

fn claims(email: Option<&str>, verified: bool) -> FederatedIdentityClaims {
    FederatedIdentityClaims {
        email: email.map(ToOwned::to_owned),
        email_verified: verified,
        name: Some(format!("Federated Person {}", Uuid::new_v4().simple())),
        preferred_username: None,
        roles: Vec::new(),
    }
}

async fn cleanup(ctx: &Ctx, user_id: &UserId) {
    // users.id is FK-referenced ON DELETE CASCADE from federated_identities.
    let _ = ctx.repo.delete(user_id).await;
}

#[tokio::test]
async fn find_federated_unknown_returns_none() {
    let ctx = setup("unknown").await;
    let found = ctx
        .repo
        .find_federated(&ctx.issuer, &ctx.external_sub)
        .await
        .expect("find_federated");
    assert!(found.is_none());
}

#[tokio::test]
async fn create_then_find_and_reuse_identity() {
    let ctx = setup("create").await;

    let email = format!("verified-{}@example.com", Uuid::new_v4().simple());
    let identity_claims = claims(Some(&email), true);
    let user = ctx
        .repo
        .find_or_create_federated(&ctx.issuer, &ctx.external_sub, &identity_claims)
        .await
        .expect("first create");

    assert_eq!(user.email, email);
    assert_eq!(user.display_name, identity_claims.name);
    assert!(user.roles.iter().any(|r| r == "user"));

    let mapped = ctx
        .repo
        .find_federated(&ctx.issuer, &ctx.external_sub)
        .await
        .expect("find_federated")
        .expect("mapping present");
    assert_eq!(mapped, user.id);

    let again = ctx
        .repo
        .find_or_create_federated(&ctx.issuer, &ctx.external_sub, &identity_claims)
        .await
        .expect("second create");
    assert_eq!(again.id, user.id, "existing identity must be reused");

    cleanup(&ctx, &user.id).await;
}

#[tokio::test]
async fn unverified_email_yields_synthetic_local_address() {
    let ctx = setup("unverified").await;

    let user = ctx
        .repo
        .find_or_create_federated(
            &ctx.issuer,
            &ctx.external_sub,
            &claims(Some("hostile@victim.com"), false),
        )
        .await
        .expect("create");

    assert_ne!(user.email, "hostile@victim.com");
    assert!(
        user.email.ends_with(".federated.local"),
        "unverified upstream email must map to a synthetic local address, got {}",
        user.email
    );

    cleanup(&ctx, &user.id).await;
}

#[tokio::test]
async fn missing_email_yields_synthetic_local_address() {
    let ctx = setup("noemail").await;

    let user = ctx
        .repo
        .find_or_create_federated(&ctx.issuer, &ctx.external_sub, &claims(None, false))
        .await
        .expect("create");

    assert!(user.email.ends_with(".federated.local"));

    cleanup(&ctx, &user.id).await;
}
