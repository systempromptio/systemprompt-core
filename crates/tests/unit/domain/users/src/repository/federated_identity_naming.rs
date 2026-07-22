//! DB-backed tests for federated-identity name, role, and synthetic-email
//! derivation branches not covered by the round-trip suite.

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
    let tag = Uuid::new_v4();
    Some(Ctx {
        repo: UserRepository::new(&pool).expect("repo"),
        issuer: format!("https://idp-{prefix}-{tag}.example.com:8443/realm"),
        external_sub: format!("sub-{prefix}-{tag}"),
    })
}

async fn cleanup(ctx: &Ctx, user_id: &UserId) {
    let _ = ctx.repo.delete(user_id).await;
}

#[tokio::test]
async fn preferred_username_wins_over_display_name() {
    let Some(ctx) = setup("username").await else {
        return;
    };
    let claims = FederatedIdentityClaims {
        email: None,
        email_verified: false,
        name: Some("Display Name".to_owned()),
        preferred_username: Some("preferred.login".to_owned()),
        roles: Vec::new(),
    };

    let user = ctx
        .repo
        .find_or_create_federated(&ctx.issuer, &ctx.external_sub, &claims)
        .await
        .expect("create");
    assert_eq!(user.name, "preferred.login");
    assert_eq!(user.display_name.as_deref(), Some("Display Name"));

    cleanup(&ctx, &user.id).await;
}

#[tokio::test]
async fn missing_username_and_name_derive_hashed_fallback() {
    let Some(ctx) = setup("fallback").await else {
        return;
    };
    let claims = FederatedIdentityClaims {
        email: None,
        email_verified: false,
        name: None,
        preferred_username: None,
        roles: Vec::new(),
    };

    let user = ctx
        .repo
        .find_or_create_federated(&ctx.issuer, &ctx.external_sub, &claims)
        .await
        .expect("create");
    assert!(
        user.name.starts_with("fed_"),
        "fallback name must be hash-derived, got {}",
        user.name
    );
    assert!(user.display_name.is_none());
    assert!(
        user.email.ends_with(".federated.local"),
        "synthetic email expected, got {}",
        user.email
    );
    assert!(
        user.email.contains("-8443.federated.local"),
        "issuer host dots and port colon must be sanitised in {}",
        user.email
    );

    cleanup(&ctx, &user.id).await;
}

#[tokio::test]
async fn upstream_roles_pass_through_when_present() {
    let Some(ctx) = setup("roles").await else {
        return;
    };
    let claims = FederatedIdentityClaims {
        email: Some("roles@example.com".to_owned()),
        email_verified: true,
        name: None,
        preferred_username: Some("role.bearer".to_owned()),
        roles: vec!["operator".to_owned(), "viewer".to_owned()],
    };

    let user = ctx
        .repo
        .find_or_create_federated(&ctx.issuer, &ctx.external_sub, &claims)
        .await
        .expect("create");
    assert_eq!(user.roles, vec!["operator".to_owned(), "viewer".to_owned()]);

    cleanup(&ctx, &user.id).await;
}
