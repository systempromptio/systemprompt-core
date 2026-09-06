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
    generate_admin_token, get_or_create_admin, resolve_admin_with_fallback,
    resolve_credentialed_user_email, resolve_local_admin,
};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{SessionId, UserId};
use systemprompt_test_fixtures::{
    closed_db_pool, ensure_test_bootstrap, fixture_database_url, fixture_db_pool,
    install_test_signing_key, seed_user_row_with_roles,
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

// The tenant fallback resolves a *different person's* admin account when the
// named session user cannot be resolved. That is intended — the operator is
// authenticated as the credential holder — but it is only safe because it
// requires a session email hint. Without that guard, any failed lookup would
// silently open a session as someone else.
mod tenant_fallback {
    use super::{pool, seed, unique};
    use systemprompt_cli::session::creation::helpers::resolve_tenant_admin_with_fallback;
    use systemprompt_cloud::CloudCredentials;
    use systemprompt_identifiers::{CloudAuthToken, Email};

    fn creds(email: &str) -> CloudCredentials {
        CloudCredentials::new(
            CloudAuthToken::new("tok"),
            "https://cloud.invalid".to_owned(),
            Email::new(email),
        )
    }

    /// `users.email` is `VARCHAR(255)`, so an over-long address fails to insert
    /// and gives the fallback a deterministic failure to react to.
    fn unresolvable_email() -> String {
        format!("{}@toolong.invalid", "x".repeat(300))
    }

    #[tokio::test]
    async fn a_resolvable_user_is_returned_without_consulting_the_credentials() {
        let pool = pool().await;
        let (_name, email) = seed(&pool, &["admin"], "active").await;
        let other = format!("{}@other.invalid", unique("creds"));

        let user = resolve_tenant_admin_with_fallback(&pool, &creds(&other), &email, Some(&email))
            .await
            .expect("a resolvable user needs no fallback");

        assert_eq!(
            user.email, email,
            "the named user was resolvable, so the credential holder must not be substituted"
        );
    }

    // Why: this is the guard. Without a session hint the caller never asserted
    // an identity, so a failed lookup must surface rather than quietly
    // resolving to whoever holds the cloud credentials.
    #[tokio::test]
    async fn without_a_session_hint_a_failure_is_reported_rather_than_falling_back() {
        let pool = pool().await;
        let holder = format!("{}@holder.invalid", unique("creds"));

        let err =
            resolve_tenant_admin_with_fallback(&pool, &creds(&holder), &unresolvable_email(), None)
                .await
                .expect_err("with no session hint the failure must propagate");

        assert!(
            !format!("{err:#}").is_empty(),
            "the refusal should carry the underlying reason"
        );
    }

    // Why: with a hint the operator did assert an identity, and the credential
    // holder is a different, authenticated person. Falling back to them is the
    // intended recovery.
    #[tokio::test]
    async fn with_a_session_hint_the_credential_holder_is_used_instead() {
        let pool = pool().await;
        let holder = format!("{}@holder.invalid", unique("creds"));
        let requested = unresolvable_email();

        let user = resolve_tenant_admin_with_fallback(
            &pool,
            &creds(&holder),
            &requested,
            Some(&requested),
        )
        .await
        .expect("the credential holder should be resolved as the fallback");

        assert_eq!(
            user.email, holder,
            "the fallback resolves the credential holder, not the requested address"
        );
        assert!(user.is_admin());
    }

    // Why: when the credential holder is the address that already failed,
    // retrying resolves nothing. The error must surface rather than the same
    // lookup being run twice.
    #[tokio::test]
    async fn no_fallback_is_attempted_when_the_credentials_name_the_same_address() {
        let pool = pool().await;
        let same = unresolvable_email();

        let err = resolve_tenant_admin_with_fallback(&pool, &creds(&same), &same, Some(&same))
            .await
            .expect_err("retrying the same address cannot succeed");

        assert!(!format!("{err:#}").is_empty());
    }
}

// A CLI context is keyed on the user and the profile, not on the session: an
// existing one is adopted and rebound to the new session id, so a user's CLI
// conversation survives logging in again. That makes the scoping the thing
// worth asserting — keyed too loosely, one operator's CLI history surfaces in
// another's.
mod cli_context {
    use super::{pool, seed};
    use systemprompt_cli::session::creation::helpers::{create_cli_context, get_or_create_admin};
    use systemprompt_database::DbPool;
    use systemprompt_identifiers::{SessionId, UserId};
    use systemprompt_test_fixtures::seed_user_session;

    /// `user_contexts.session_id` is a foreign key, so the session row has to
    /// exist before a context can point at it.
    async fn session_for(pool: &DbPool, user_id: &UserId) -> SessionId {
        let session = SessionId::generate();
        seed_user_session(pool, user_id, &session)
            .await
            .expect("seed session");
        session
    }

    #[tokio::test]
    async fn the_same_user_and_profile_keep_one_context_across_sessions() {
        let pool = pool().await;
        let (_name, email) = seed(&pool, &["admin"], "active").await;
        let user = get_or_create_admin(&pool, &email, "test")
            .await
            .expect("resolve admin");

        let one = session_for(&pool, &user.id).await;
        let two = session_for(&pool, &user.id).await;
        let first = create_cli_context(pool.clone(), &user, &one, "prof-a")
            .await
            .expect("first context");
        let second = create_cli_context(pool.clone(), &user, &two, "prof-a")
            .await
            .expect("second context");

        assert_eq!(
            first, second,
            "logging in again must continue the same CLI conversation, not start a second"
        );
    }

    // Why: profiles address different deployments. Sharing one context between
    // them would show an operator the history from a profile they are not
    // currently pointed at.
    #[tokio::test]
    async fn different_profiles_get_different_contexts() {
        let pool = pool().await;
        let (_name, email) = seed(&pool, &["admin"], "active").await;
        let user = get_or_create_admin(&pool, &email, "test")
            .await
            .expect("resolve admin");
        let session = session_for(&pool, &user.id).await;

        let a = create_cli_context(pool.clone(), &user, &session, "prof-a")
            .await
            .expect("context a");
        let b = create_cli_context(pool.clone(), &user, &session, "prof-b")
            .await
            .expect("context b");

        assert_ne!(a, b, "each profile keeps its own CLI conversation");
    }

    // Why: this is the isolation that matters. A context shared between users
    // would surface one operator's CLI history to another.
    #[tokio::test]
    async fn different_users_never_share_a_context() {
        let pool = pool().await;
        let (_n1, email_one) = seed(&pool, &["admin"], "active").await;
        let (_n2, email_two) = seed(&pool, &["admin"], "active").await;
        let one = get_or_create_admin(&pool, &email_one, "test")
            .await
            .expect("resolve first admin");
        let two = get_or_create_admin(&pool, &email_two, "test")
            .await
            .expect("resolve second admin");

        let session_one = session_for(&pool, &one.id).await;
        let session_two = session_for(&pool, &two.id).await;
        let context_one = create_cli_context(pool.clone(), &one, &session_one, "shared")
            .await
            .expect("context for the first user");
        let context_two = create_cli_context(pool.clone(), &two, &session_two, "shared")
            .await
            .expect("context for the second user");

        assert_ne!(
            context_one, context_two,
            "two operators on the same profile name must not share a CLI context"
        );
    }
}

// The two resolvers below sit in front of `get_or_create_admin` and decide
// which address it is asked for: the hint if there is one, cloud credentials
// otherwise, with a fallback that retries under the credentialed address when a
// hinted lookup fails.

#[tokio::test]
async fn a_session_hint_that_is_not_an_address_is_refused_before_any_lookup() {
    let err = resolve_credentialed_user_email(Some("not an email at all"))
        .await
        .expect_err("a malformed hint must not reach the database");

    assert!(
        format!("{err:#}").contains("not a valid email address"),
        "the refusal must name the hint as the problem, got: {err:#}"
    );
}

#[tokio::test]
async fn a_well_formed_session_hint_is_used_verbatim() {
    let email = resolve_credentialed_user_email(Some("hinted@sessadmin.invalid"))
        .await
        .expect("a valid hint needs no cloud credentials at all");

    assert_eq!(email.as_str(), "hinted@sessadmin.invalid");
}

#[tokio::test]
async fn a_lookup_with_no_hint_and_no_credentials_says_to_authenticate() {
    // skip-ok: no database, so nothing to act on
    let Err(err) = resolve_credentialed_user_email(None).await else {
        return;
    };

    assert!(
        format!("{err:#}").contains("cloud auth login"),
        "without credentials the operator must be told how to get them, got: {err:#}"
    );
}

#[tokio::test]
async fn an_address_with_no_user_behind_it_is_provisioned_as_an_admin() {
    let pool = pool().await;
    let email = format!("{}@sessadmin.invalid", unique("fallback"));

    let user = resolve_admin_with_fallback(&pool, &email, None, "local")
        .await
        .expect("an address with no user is provisioned rather than refused");

    assert_eq!(user.email, email);
    assert!(
        user.is_admin(),
        "a provisioned session user must hold the admin role, got {:?}",
        user.roles
    );
}

// Why: the fallback arm fires only when the *lookup* fails, which no address
// can cause — `get_or_create_admin` provisions whatever it is given. A closed
// pool is the failure it is actually written for.
#[tokio::test]
async fn a_hinted_lookup_that_fails_falls_back_and_still_reports_the_original_failure() {
    let pool = closed_db_pool().await;

    let err = resolve_admin_with_fallback(
        &pool,
        "hinted@sessadmin.invalid",
        Some("hinted@sessadmin.invalid"),
        "local",
    )
    .await
    .expect_err("a closed pool cannot resolve or provision anyone");

    assert!(
        format!("{err:#}").contains("Failed to query user by email"),
        "the original lookup failure must survive the fallback, got: {err:#}"
    );
}

#[tokio::test]
async fn a_hintless_lookup_that_fails_is_reported_without_a_fallback() {
    let pool = closed_db_pool().await;

    let err = resolve_admin_with_fallback(&pool, "plain@sessadmin.invalid", None, "local")
        .await
        .expect_err("a closed pool cannot resolve anyone");

    assert!(
        format!("{err:#}").contains("Failed to query user by email"),
        "the lookup failure must be reported as-is, got: {err:#}"
    );
}
