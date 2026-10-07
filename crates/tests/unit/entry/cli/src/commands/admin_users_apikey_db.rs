//! `admin users apikey` — issuing, listing and revoking personal access
//! tokens.
//!
//! An API key is a bearer credential, so the interesting behaviour is what
//! happens around the secret: it is returned exactly once at issue and never
//! again, only its prefix is stored for listing, and revocation has to be
//! visible on the row rather than silently doing nothing.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::admin::users::{self, UsersCommands};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{ScopeDimension, UserId};
use systemprompt_models::attribution::ScopeBinding;
use systemprompt_test_fixtures::{
    seed_user_row, test_app_context, test_database_url, test_db_pool,
};
use systemprompt_users::ApiKeyLimits;
use uuid::Uuid;

#[derive(Debug, Parser)]
struct UsersHarness {
    #[command(subcommand)]
    cmd: UsersCommands,
}

fn parse(args: &[&str]) -> UsersCommands {
    UsersHarness::try_parse_from(std::iter::once("users").chain(args.iter().copied()))
        .unwrap_or_else(|e| panic!("parse {args:?}: {e}"))
        .cmd
}


fn ctx(pool: &DbPool) -> CommandContext {
    let url = test_database_url();
    CommandContext::with_app_context(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
        test_app_context(pool, &url),
    )
}

async fn seeded_user(pool: &DbPool) -> String {
    let id = format!("apikey-user-{}", Uuid::new_v4().simple());
    seed_user_row(pool, &UserId::new(&id), &format!("{id}@apikey.invalid"))
        .await
        .expect("seed user");
    id
}

async fn run(pool: &DbPool, args: &[&str]) -> anyhow::Result<()> {
    users::execute(parse(args), &ctx(pool)).await
}

struct StoredKey {
    id: String,
    name: String,
    key_prefix: String,
    key_hash: String,
    expires_at: Option<chrono::DateTime<chrono::Utc>>,
    revoked_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// `execute` renders to stdout, so the row is where the outcome is visible.
async fn stored_keys(pool: &DbPool, user: &str) -> Vec<StoredKey> {
    let p = pool.pool();
    sqlx::query_as::<
        _,
        (
            String,
            String,
            String,
            String,
            Option<chrono::DateTime<chrono::Utc>>,
            Option<chrono::DateTime<chrono::Utc>>,
        ),
    >(
        "SELECT id, name, key_prefix, key_hash, expires_at, revoked_at \
         FROM user_api_keys WHERE user_id = $1 ORDER BY created_at",
    )
    .bind(user)
    .fetch_all(&*p)
    .await
    .expect("read api keys")
    .into_iter()
    .map(
        |(id, name, key_prefix, key_hash, expires_at, revoked_at)| StoredKey {
            id,
            name,
            key_prefix,
            key_hash,
            expires_at,
            revoked_at,
        },
    )
    .collect()
}

// Why: the secret is shown once and never stored. Only a prefix, for
// identifying the key in a listing, and a hash, for verifying it — so a
// database read must never yield anything a caller could authenticate with.
#[tokio::test]
async fn issuing_stores_a_hash_and_a_prefix_but_never_the_secret() {
    let pool = test_db_pool().await;
    let user = seeded_user(&pool).await;

    run(
        &pool,
        &["api-key", "issue", "--user", &user, "--name", "ci"],
    )
    .await
    .expect("issue");

    let keys = stored_keys(&pool, &user).await;
    assert_eq!(keys.len(), 1, "one issue, one row");
    let key = &keys[0];
    assert_eq!(key.name, "ci");
    assert!(!key.key_hash.is_empty(), "the key must be verifiable");
    assert!(
        !key.key_prefix.is_empty(),
        "the prefix is how an operator identifies the key to revoke"
    );
    assert_ne!(
        key.key_hash, key.key_prefix,
        "storing the same value twice would mean one of them is the secret"
    );
    assert!(key.revoked_at.is_none(), "a fresh key is live");
}

#[tokio::test]
async fn revoking_marks_the_row_rather_than_deleting_it() {
    let pool = test_db_pool().await;
    let user = seeded_user(&pool).await;

    run(
        &pool,
        &["api-key", "issue", "--user", &user, "--name", "ci"],
    )
    .await
    .expect("issue");
    let id = stored_keys(&pool, &user).await[0].id.clone();

    run(&pool, &["api-key", "revoke", "--user", &user, "--id", &id])
        .await
        .expect("revoke");

    let keys = stored_keys(&pool, &user).await;
    assert_eq!(
        keys.len(),
        1,
        "the row survives revocation so the key remains auditable"
    );
    assert!(
        keys[0].revoked_at.is_some(),
        "revocation must be visible on the row, or the key is still live"
    );
}

// Why: a nameless key is unidentifiable in the listing, which is the only
// place an operator can see what exists to revoke. Guarded twice — the command
// checks before calling and the service checks again — so this asserts the
// behaviour rather than either copy of it: removing the CLI guard alone does
// not make it pass.
#[tokio::test]
async fn a_key_with_a_blank_name_is_refused() {
    let pool = test_db_pool().await;
    let user = seeded_user(&pool).await;

    let err = run(
        &pool,
        &["api-key", "issue", "--user", &user, "--name", "   "],
    )
    .await
    .expect_err("a whitespace-only name must not be accepted");

    assert!(
        format!("{err:#}").to_lowercase().contains("name"),
        "the refusal should say the name is the problem: {err:#}"
    );
}

// Why: `--expires` is an operator-typed timestamp. A value that does not parse
// has to be rejected at the boundary, because the alternative is storing a key
// with an expiry nobody intended — most likely none at all.
#[tokio::test]
async fn an_expiry_that_is_not_rfc3339_is_rejected_at_parse_time() {
    let bad = UsersHarness::try_parse_from([
        "users",
        "api-key",
        "issue",
        "--user",
        "someone",
        "--name",
        "ci",
        "--expires",
        "next tuesday",
    ]);

    let err = bad.expect_err("an unparseable expiry must not reach the service");
    assert!(
        format!("{err}").contains("RFC 3339"),
        "the parse error should name the format expected: {err}"
    );
}

#[tokio::test]
async fn an_explicit_rfc3339_expiry_is_accepted() {
    let pool = test_db_pool().await;
    let user = seeded_user(&pool).await;

    run(
        &pool,
        &[
            "api-key",
            "issue",
            "--user",
            &user,
            "--name",
            "expiring",
            "--expires",
            "2030-01-01T00:00:00Z",
        ],
    )
    .await
    .expect("an RFC 3339 expiry should be accepted");

    let keys = stored_keys(&pool, &user).await;
    assert_eq!(
        keys[0].expires_at.map(|t| t.to_rfc3339()),
        Some("2030-01-01T00:00:00+00:00".to_owned()),
        "the expiry the operator typed is the expiry stored"
    );
}

// Why: revoking a key that is not there must report rather than succeed
// quietly. An operator who mistypes an id and sees success believes a live
// credential has been withdrawn when it has not.
#[tokio::test]
async fn revoking_a_key_that_does_not_exist_is_reported() {
    let pool = test_db_pool().await;
    let user = seeded_user(&pool).await;

    let err = run(
        &pool,
        &[
            "api-key",
            "revoke",
            "--user",
            &user,
            "--id",
            "key-that-does-not-exist",
        ],
    )
    .await
    .expect_err("revoking an unknown key must not report success");

    assert!(
        !format!("{err:#}").is_empty(),
        "the refusal should carry a reason"
    );
}

#[tokio::test]
async fn listing_a_user_with_no_keys_is_an_empty_report_rather_than_an_error() {
    let pool = test_db_pool().await;
    let user = seeded_user(&pool).await;

    run(&pool, &["api-key", "list", "--user", &user])
        .await
        .expect("a user with no keys lists nothing, which is not a failure");

    assert!(stored_keys(&pool, &user).await.is_empty());
}

fn issue_args(extra: &[&str]) -> users::ApiKeyIssueArgs {
    let mut args = vec!["api-key", "issue", "--user", "someone", "--name", "ci"];
    args.extend_from_slice(extra);
    match parse(&args) {
        UsersCommands::ApiKey(users::ApiKeyCommands::Issue(issue)) => issue,
        other => panic!("expected api-key issue, got {other:?}"),
    }
}

#[test]
fn no_limit_flags_issue_an_unlimited_unscoped_key() {
    let args = issue_args(&[]);
    assert_eq!(args.limits(), ApiKeyLimits::default());
    assert!(args.scopes.is_empty());
}

#[test]
fn every_limit_flag_lands_on_the_typed_limits() {
    let args = issue_args(&[
        "--model",
        "claude-a",
        "--model",
        "claude-b",
        "--budget-microdollars",
        "5000000",
        "--max-requests",
        "100",
        "--window-seconds",
        "3600",
        "--scope",
        "project=p-primary",
        "--scope",
        "cost_centre=cc-9",
    ]);
    assert_eq!(
        args.limits(),
        ApiKeyLimits {
            model_allowlist: Some(vec!["claude-a".to_owned(), "claude-b".to_owned()]),
            budget_microdollars: Some(5_000_000),
            max_requests: Some(100),
            request_window_seconds: Some(3600),
        }
    );
    assert_eq!(
        args.scopes,
        vec![
            ScopeBinding {
                dimension: ScopeDimension::new("project"),
                value: "p-primary".to_owned(),
            },
            ScopeBinding {
                dimension: ScopeDimension::new("cost_centre"),
                value: "cc-9".to_owned(),
            },
        ]
    );
}

fn issue_parse_error(extra: &[&str]) -> String {
    let mut args = vec!["users", "api-key", "issue", "--user", "u", "--name", "ci"];
    args.extend_from_slice(extra);
    UsersHarness::try_parse_from(args)
        .expect_err("a malformed flag must be rejected at parse time")
        .to_string()
}

#[test]
fn a_scope_without_a_separator_is_rejected() {
    let err = issue_parse_error(&["--scope", "project"]);
    assert!(err.contains("DIMENSION=VALUE"), "{err}");
}

#[test]
fn a_scope_with_an_empty_value_is_rejected() {
    let err = issue_parse_error(&["--scope", "project="]);
    assert!(err.contains("empty"), "{err}");
}

#[test]
fn a_scope_with_an_invalid_dimension_is_rejected() {
    let err = issue_parse_error(&["--scope", "Project=p"]);
    assert!(err.contains("scope dimension"), "{err}");
}

#[test]
fn non_positive_ceilings_are_rejected() {
    issue_parse_error(&["--max-requests", "0"]);
    issue_parse_error(&["--window-seconds", "-1"]);
    issue_parse_error(&["--budget-microdollars", "-5"]);
}

// Why: no subject attribute provider is registered in this test binary, so a
// bound dimension cannot be verified and must refuse issuance rather than
// store an unverified binding.
#[tokio::test]
async fn a_scope_on_an_unregistered_dimension_refuses_issuance() {
    let pool = test_db_pool().await;
    let user = seeded_user(&pool).await;

    let err = run(
        &pool,
        &[
            "api-key",
            "issue",
            "--user",
            &user,
            "--name",
            "ci",
            "--scope",
            "project=p1",
        ],
    )
    .await
    .expect_err("an unverifiable scope must not be issued");

    assert!(
        format!("{err:#}").contains("unknown scope dimension"),
        "{err:#}"
    );
    assert!(
        stored_keys(&pool, &user).await.is_empty(),
        "nothing is stored"
    );
}

#[tokio::test]
async fn limits_given_on_the_command_line_are_persisted() {
    let pool = test_db_pool().await;
    let user = seeded_user(&pool).await;

    run(
        &pool,
        &[
            "api-key",
            "issue",
            "--user",
            &user,
            "--name",
            "limited",
            "--model",
            "claude-a",
            "--max-requests",
            "10",
            "--window-seconds",
            "60",
        ],
    )
    .await
    .expect("issue with limits");

    let (allowlist, max_requests, window): (Option<Vec<String>>, Option<i32>, Option<i32>) =
        sqlx::query_as(
            "SELECT model_allowlist, max_requests, request_window_seconds \
             FROM user_api_keys WHERE user_id = $1",
        )
        .bind(&user)
        .fetch_one(&*pool.pool())
        .await
        .expect("read limits");
    assert_eq!(allowlist, Some(vec!["claude-a".to_owned()]));
    assert_eq!(max_requests, Some(10));
    assert_eq!(window, Some(60));
}
