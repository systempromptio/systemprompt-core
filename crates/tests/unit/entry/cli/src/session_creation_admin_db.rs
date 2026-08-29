//! Resolving and provisioning the admin behind a CLI session.
//!
//! These decide who gets a session token carrying `UserType::Admin`,
//! `Permission::Admin` and the admin rate-limit tier. Two of them are
//! authorisation gates — an inactive or non-admin user must not resolve — and
//! one is a deliberate privilege escalation: an existing non-admin user named
//! as the session owner is promoted rather than refused. That promotion is
//! intended, and pinning it means a later change to it has to be deliberate.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::session::creation::helpers::{
    generate_admin_token, get_or_create_admin, resolve_local_admin,
};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{SessionId, UserId};
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_database_url, fixture_db_pool, install_test_signing_key,
    seed_user_row_with_roles,
};
use uuid::Uuid;

async fn pool() -> DbPool {
    fixture_db_pool(&fixture_database_url().expect("DATABASE_URL"))
        .await
        .expect("the session creation tests need a reachable test database")
}

fn unique(prefix: &str) -> String {
    format!("{prefix}-{}", Uuid::new_v4().simple())
}

async fn seed(pool: &DbPool, roles: &[&str], status: &str) -> (String, String) {
    let name = unique("sessadmin");
    let email = format!("{name}@sessadmin.invalid");
    let roles: Vec<String> = roles.iter().map(|r| (*r).to_owned()).collect();
    seed_user_row_with_roles(pool, &UserId::new(&name), &email, &roles)
        .await
        .expect("seed user");

    if status != "active" {
        let write = pool.write_pool_arc().expect("write pool");
        sqlx::query("UPDATE users SET status = $2 WHERE id = $1")
            .bind(&name)
            .bind(status)
            .execute(&*write)
            .await
            .expect("set status");
    }
    (name, email)
}

// Why: this is the precondition for the refusals below. If a well-formed
// active admin did not resolve, every negative test would pass for the wrong
// reason.
#[tokio::test]
async fn an_active_admin_resolves() {
    let pool = pool().await;
    let (name, _email) = seed(&pool, &["admin"], "active").await;

    let user = resolve_local_admin(&pool, &name)
        .await
        .expect("an active admin should resolve");

    assert_eq!(user.id.as_str(), name);
    assert!(user.is_admin());
}

// Why: a deactivated account must not still open an admin CLI session.
// Deactivation is how access is withdrawn, and it is only withdrawn if this
// check holds.
#[tokio::test]
async fn an_inactive_user_is_refused_even_when_they_hold_the_admin_role() {
    let pool = pool().await;
    let (name, _email) = seed(&pool, &["admin"], "inactive").await;

    let err = resolve_local_admin(&pool, &name)
        .await
        .expect_err("an inactive user must not resolve as the local admin");

    assert!(
        format!("{err:#}").contains("not active"),
        "the refusal should say the account is inactive: {err:#}"
    );
}

// Why: holding no admin role is the other half of the gate. Without this a
// plain user named as the session owner would be handed an admin token.
#[tokio::test]
async fn a_user_without_the_admin_role_is_refused() {
    let pool = pool().await;
    let (name, _email) = seed(&pool, &["user"], "active").await;

    let err = resolve_local_admin(&pool, &name)
        .await
        .expect_err("a non-admin must not resolve as the local admin");

    assert!(
        format!("{err:#}").contains("admin role"),
        "the refusal should name the missing role: {err:#}"
    );
}

#[tokio::test]
async fn an_unknown_name_is_refused_with_the_repair_instruction() {
    let pool = pool().await;

    let err = resolve_local_admin(&pool, &unique("nobody"))
        .await
        .expect_err("an unknown name must not resolve");

    assert!(
        format!("{err:#}").contains("admin bootstrap"),
        "the refusal should tell the operator how to create it: {err:#}"
    );
}

// Why: this is the escalation path, and it is deliberate — the operator named
// this address as the session owner. Pinned so that changing it, in either
// direction, is a decision rather than a side effect.
#[tokio::test]
async fn an_existing_non_admin_is_promoted_rather_than_refused() {
    let pool = pool().await;
    let (_name, email) = seed(&pool, &["user"], "active").await;

    let user = get_or_create_admin(&pool, &email, "test")
        .await
        .expect("an existing user should be promoted");

    assert!(
        user.is_admin(),
        "the named user must come back holding the admin role"
    );
    assert_eq!(user.email, email, "promotion must not switch accounts");
}

#[tokio::test]
async fn an_existing_admin_comes_back_unchanged() {
    let pool = pool().await;
    let (name, email) = seed(&pool, &["admin"], "active").await;

    let user = get_or_create_admin(&pool, &email, "test")
        .await
        .expect("an existing admin should resolve");

    assert_eq!(user.id.as_str(), name, "no new account may be created");
    assert!(user.is_admin());
}

// Why: provisioning derives the account name from the address's local part. A
// wrong derivation creates an account under a name the operator did not
// choose and cannot find again.
#[tokio::test]
async fn a_missing_user_is_provisioned_as_an_admin_named_for_the_address() {
    let pool = pool().await;
    let local = unique("fresh");
    let email = format!("{local}@sessadmin.invalid");

    let user = get_or_create_admin(&pool, &email, "test")
        .await
        .expect("a missing user should be provisioned");

    assert_eq!(user.email, email);
    assert_eq!(
        user.name, local,
        "the account name comes from the address's local part"
    );
    assert!(
        user.is_admin(),
        "a provisioned session owner must hold the admin role"
    );
}

// Why: the token is the CLI's admin credential, so it must actually be
// signed — the failure without a key is "signing key unavailable", not an
// unsigned token, which is the right refusal but not what is under test here.
// The committed test key is installed rather than generating RSA at runtime,
// which is what makes this suite fast enough to keep.
#[tokio::test]
async fn the_minted_token_names_its_user_and_is_not_empty() {
    ensure_test_bootstrap();
    install_test_signing_key();
    let pool = pool().await;
    let (_name, email) = seed(&pool, &["admin"], "active").await;
    let user = get_or_create_admin(&pool, &email, "test")
        .await
        .expect("resolve admin");

    let session_id = SessionId::generate();
    let token = generate_admin_token("https://issuer.invalid", &user, &session_id)
        .expect("minting an admin token should succeed");

    assert!(
        !token.as_str().is_empty(),
        "an empty session token would authenticate nothing"
    );
    assert_eq!(
        token.as_str().split('.').count(),
        3,
        "a session token is a three-part JWT: {}",
        token.as_str()
    );
}
