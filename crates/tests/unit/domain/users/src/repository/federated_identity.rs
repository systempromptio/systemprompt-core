//! DB-backed tests for the federated-identity repository
//! (`find_federated`, `find_or_create_federated`).

use systemprompt_identifiers::UserId;
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_database_url, fixture_db_pool};
use systemprompt_traits::FederatedIdentityClaims;
use systemprompt_users::UserRepository;
use uuid::Uuid;

struct Ctx {
    repo: UserRepository,
    issuer: String,
    external_sub: String,
}

async fn setup(prefix: &str) -> Option<Ctx> {
    let url = fixture_database_url().ok()?;
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = UserRepository::new(&pool).expect("repo");
    let tag = Uuid::new_v4();
    Some(Ctx {
        repo,
        issuer: format!("https://idp-{prefix}-{tag}.example.com/realm"),
        external_sub: format!("sub-{prefix}-{tag}"),
    })
}

fn claims(email: Option<&str>, verified: bool) -> FederatedIdentityClaims {
    FederatedIdentityClaims {
        email: email.map(ToOwned::to_owned),
        email_verified: verified,
        name: Some("Federated Person".to_owned()),
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
    let Some(ctx) = setup("unknown").await else {
        return;
    };
    let found = ctx
        .repo
        .find_federated(&ctx.issuer, &ctx.external_sub)
        .await
        .expect("find_federated");
    assert!(found.is_none());
}

#[tokio::test]
async fn create_then_find_and_reuse_identity() {
    let Some(ctx) = setup("create").await else {
        return;
    };

    let user = ctx
        .repo
        .find_or_create_federated(
            &ctx.issuer,
            &ctx.external_sub,
            &claims(Some("verified@example.com"), true),
        )
        .await
        .expect("first create");

    assert_eq!(user.email, "verified@example.com");
    assert_eq!(user.display_name.as_deref(), Some("Federated Person"));
    assert!(!user.roles.is_empty());

    let mapped = ctx
        .repo
        .find_federated(&ctx.issuer, &ctx.external_sub)
        .await
        .expect("find_federated")
        .expect("mapping present");
    assert_eq!(mapped, user.id);

    let again = ctx
        .repo
        .find_or_create_federated(
            &ctx.issuer,
            &ctx.external_sub,
            &claims(Some("verified@example.com"), true),
        )
        .await
        .expect("second create");
    assert_eq!(again.id, user.id, "existing identity must be reused");

    cleanup(&ctx, &user.id).await;
}

#[tokio::test]
async fn unverified_email_yields_synthetic_local_address() {
    let Some(ctx) = setup("unverified").await else {
        return;
    };

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
    let Some(ctx) = setup("noemail").await else {
        return;
    };

    let user = ctx
        .repo
        .find_or_create_federated(&ctx.issuer, &ctx.external_sub, &claims(None, false))
        .await
        .expect("create");

    assert!(user.email.ends_with(".federated.local"));

    cleanup(&ctx, &user.id).await;
}
