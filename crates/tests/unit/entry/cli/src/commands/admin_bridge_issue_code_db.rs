//! `admin bridge issue-code` — minting a device-link code.
//!
//! The code lets a bridge device claim a user's identity, so what reaches the
//! database matters more than what the command prints. The row is bound to the
//! *resolved* user, and it expires — a device-link code without an expiry is a
//! permanent credential.
//!
//! That only a hash is stored is asserted in the oauth suite, not here. The
//! code and its sha256 are both 64 hex characters, so from this side — where
//! `execute` renders the code and returns `Ok(())` — the two are
//! indistinguishable by shape. A test asserting "looks like a hash" passes
//! just as happily when the plaintext is stored; verified by storing the
//! plaintext and watching such a test stay green.
//!
//! These drive the public `execute`, which renders and returns `Ok(())`, so
//! every assertion reads the row rather than the return value.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use chrono::{DateTime, Utc};
use clap::Parser;
use systemprompt_cli::admin::bridge::{self, BridgeCommands};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_database::DbPool;
use systemprompt_identifiers::UserId;
use systemprompt_test_fixtures::{
    fixture_app_context, fixture_database_url, fixture_db_pool, seed_user_row,
};
use uuid::Uuid;

#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    cmd: BridgeCommands,
}

fn parse(args: &[&str]) -> BridgeCommands {
    Harness::try_parse_from(std::iter::once("bridge").chain(args.iter().copied()))
        .unwrap_or_else(|e| panic!("parse {args:?}: {e}"))
        .cmd
}

async fn pool() -> DbPool {
    fixture_db_pool(&fixture_database_url().expect("DATABASE_URL"))
        .await
        .expect("the bridge issue-code tests need a reachable test database")
}

fn ctx(pool: &DbPool) -> CommandContext {
    let url = fixture_database_url().expect("DATABASE_URL");
    CommandContext::with_app_context(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
        fixture_app_context(pool, &url).expect("app context"),
    )
}

async fn seeded_user(pool: &DbPool) -> (String, String) {
    let id = format!("bridgeuser-{}", Uuid::new_v4().simple());
    let email = format!("{id}@bridge.invalid");
    seed_user_row(pool, &UserId::new(&id), &email)
        .await
        .expect("seed user");
    (id, email)
}

struct Issued {
    code_hash: String,
    expires_at: DateTime<Utc>,
    consumed_at: Option<DateTime<Utc>>,
}

async fn codes_for(pool: &DbPool, user: &str) -> Vec<Issued> {
    let p = pool.pool_arc().expect("read pool");
    sqlx::query_as::<_, (String, DateTime<Utc>, Option<DateTime<Utc>>)>(
        "SELECT code_hash, expires_at, consumed_at FROM bridge_exchange_codes \
         WHERE user_id = $1 ORDER BY created_at",
    )
    .bind(user)
    .fetch_all(&*p)
    .await
    .expect("read exchange codes")
    .into_iter()
    .map(|(code_hash, expires_at, consumed_at)| Issued {
        code_hash,
        expires_at,
        consumed_at,
    })
    .collect()
}

async fn issue(pool: &DbPool, reference: &str) -> anyhow::Result<()> {
    bridge::execute(parse(&["issue-code", "--user-id", reference]), &ctx(pool)).await
}

// Why: without an expiry the code is a permanent credential — anyone who ever
// sees it can link a device later.
#[tokio::test]
async fn the_code_expires_rather_than_lasting_forever() {
    let pool = pool().await;
    let (user, _email) = seeded_user(&pool).await;

    issue(&pool, &user).await.expect("issue code");

    let issued = &codes_for(&pool, &user).await[0];
    let now = Utc::now();

    assert!(
        issued.expires_at > now,
        "a code that is already expired cannot be used at all"
    );
    assert!(
        issued.expires_at < now + chrono::Duration::hours(1),
        "a device-link code must be short-lived, got {}",
        issued.expires_at
    );
    assert!(
        issued.consumed_at.is_none(),
        "a freshly issued code has not been consumed"
    );
}

// Why: the reference may be an id, an email, or a name, and the row must bind
// to the user it resolved to. Binding to the raw input would create a code for
// an identifier that is not a user id.
#[tokio::test]
async fn issuing_by_email_binds_the_code_to_the_resolved_user() {
    let pool = pool().await;
    let (user, email) = seeded_user(&pool).await;

    issue(&pool, &email).await.expect("issue by email");

    assert_eq!(
        codes_for(&pool, &user).await.len(),
        1,
        "the code must be bound to the resolved user id, not the email"
    );
}

#[tokio::test]
async fn issuing_by_id_and_by_email_reach_the_same_user() {
    let pool = pool().await;
    let (user, email) = seeded_user(&pool).await;

    issue(&pool, &user).await.expect("issue by id");
    issue(&pool, &email).await.expect("issue by email");

    assert_eq!(
        codes_for(&pool, &user).await.len(),
        2,
        "both references name one user, so both codes belong to them"
    );
}

// Why: minting for an unresolvable reference would create a credential bound
// to nothing, or to a row a later user could occupy.
#[tokio::test]
async fn an_unknown_reference_is_refused_and_mints_nothing() {
    let pool = pool().await;
    let absent = format!("nobody-{}", Uuid::new_v4().simple());

    let err = issue(&pool, &absent)
        .await
        .expect_err("an unknown user must not receive a device-link code");

    assert!(
        format!("{err:#}").contains(&absent),
        "the refusal should name what could not be resolved: {err:#}"
    );
    assert!(codes_for(&pool, &absent).await.is_empty());
}

#[tokio::test]
async fn an_empty_reference_is_refused() {
    let pool = pool().await;

    let err = issue(&pool, "   ")
        .await
        .expect_err("a blank reference must not resolve to anyone");

    assert!(
        format!("{err:#}").contains("empty"),
        "the refusal should say the reference was empty: {err:#}"
    );
}

// Why: two issues must not collide on the primary key or overwrite each
// other. An operator re-issuing after a failed link needs the new code to
// exist alongside the old one until it expires.
#[tokio::test]
async fn reissuing_adds_a_second_code_rather_than_replacing_the_first() {
    let pool = pool().await;
    let (user, _email) = seeded_user(&pool).await;

    issue(&pool, &user).await.expect("first issue");
    issue(&pool, &user).await.expect("second issue");

    let codes = codes_for(&pool, &user).await;
    assert_eq!(codes.len(), 2);
    assert_ne!(
        codes[0].code_hash, codes[1].code_hash,
        "two issues must mint different codes"
    );
}

// `enroll-cert` registers the certificate fingerprint a bridge device presents
// to authenticate. `DeviceCertService` is well covered on its own — including
// normalisation and every rejection — so these assert the part only the
// command decides: which user the fingerprint ends up attached to.
mod enroll_cert {
    use super::{codes_for, ctx, parse, pool, seeded_user};
    use chrono::{DateTime, Utc};
    use systemprompt_cli::admin::bridge;
    use systemprompt_database::DbPool;
    use uuid::Uuid;

    /// `user_device_certs.fingerprint` is UNIQUE across the shared test
    /// database, and fingerprints are normalised to lower case before storage.
    /// A fixed value collides with any other suite using the same one — the
    /// users-crate service tests enrol `'a' * 64`, which is what a seeded
    /// constant here collided with.
    fn fingerprint() -> String {
        format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
    }

    struct Enrolled {
        fingerprint: String,
        label: String,
        revoked_at: Option<DateTime<Utc>>,
    }

    async fn certs_for(pool: &DbPool, user: &str) -> Vec<Enrolled> {
        let p = pool.pool_arc().expect("read pool");
        sqlx::query_as::<_, (String, String, Option<DateTime<Utc>>)>(
            "SELECT fingerprint, label, revoked_at FROM user_device_certs \
             WHERE user_id = $1 ORDER BY enrolled_at",
        )
        .bind(user)
        .fetch_all(&*p)
        .await
        .expect("read device certs")
        .into_iter()
        .map(|(fingerprint, label, revoked_at)| Enrolled {
            fingerprint,
            label,
            revoked_at,
        })
        .collect()
    }

    async fn enroll(
        pool: &DbPool,
        reference: &str,
        fingerprint: &str,
        label: &str,
    ) -> anyhow::Result<()> {
        bridge::execute(
            parse(&[
                "enroll-cert",
                "--user-id",
                reference,
                "--fingerprint",
                fingerprint,
                "--label",
                label,
            ]),
            &ctx(pool),
        )
        .await
    }

    // Why: the reference may be an id, an email or a name. Attaching the
    // fingerprint to the raw input rather than the resolved user would enrol a
    // device against something that is not a user id.
    #[tokio::test]
    async fn enrolling_by_email_attaches_the_cert_to_the_resolved_user() {
        let pool = pool().await;
        let (user, email) = seeded_user(&pool).await;
        let fp = fingerprint();

        enroll(&pool, &email, &fp, "laptop")
            .await
            .expect("enrol by email");

        let certs = certs_for(&pool, &user).await;
        assert_eq!(certs.len(), 1, "the cert belongs to the resolved user id");
        assert_eq!(certs[0].fingerprint, fp);
        assert_eq!(certs[0].label, "laptop");
        assert!(
            certs[0].revoked_at.is_none(),
            "a freshly enrolled cert is active"
        );
    }

    // Why: the fingerprint is normalised before storage, so a device
    // presenting the same value in a different case must match the row that
    // was enrolled — otherwise the device authenticates once and never again.
    #[tokio::test]
    async fn an_uppercase_fingerprint_is_stored_in_its_normalised_form() {
        let pool = pool().await;
        let (user, _email) = seeded_user(&pool).await;
        let upper = fingerprint().to_uppercase();

        enroll(&pool, &user, &upper, "desktop")
            .await
            .expect("enrol uppercase");

        assert_eq!(
            certs_for(&pool, &user).await[0].fingerprint,
            upper.to_lowercase(),
            "the stored fingerprint must be the normalised one the verifier looks up"
        );
    }

    // Why: a malformed fingerprint must be refused rather than stored. A row
    // that can never match a real certificate is a permanent dead enrolment.
    #[tokio::test]
    async fn a_fingerprint_of_the_wrong_length_is_refused_and_stores_nothing() {
        let pool = pool().await;
        let (user, _email) = seeded_user(&pool).await;

        let err = enroll(&pool, &user, "abc123", "short")
            .await
            .expect_err("a 6-character fingerprint is not a SHA-256 digest");

        assert!(
            format!("{err:#}").contains("64"),
            "the refusal should name the expected length: {err:#}"
        );
        assert!(certs_for(&pool, &user).await.is_empty());
    }

    #[tokio::test]
    async fn an_unknown_user_is_refused_and_enrols_nothing() {
        let pool = pool().await;
        let absent = format!("nobody-{}", Uuid::new_v4().simple());

        let err = enroll(&pool, &absent, &fingerprint(), "ghost")
            .await
            .expect_err("an unknown user must not receive a device enrolment");

        assert!(format!("{err:#}").contains(&absent));
        assert!(certs_for(&pool, &absent).await.is_empty());
        assert!(codes_for(&pool, &absent).await.is_empty());
    }
}
