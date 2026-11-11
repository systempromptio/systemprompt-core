//! `admin users federated-rekey`: which issuer the links move to, that the
//! command is destructive and refuses an implicitly selected cloud profile,
//! and that a dry run reports without writing while `--apply` moves the rows.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::admin::users::federated_rekey::{
    RekeyChoice, id_jag_candidates, resolve_choice, run,
};
use systemprompt_cli::args::{Cli, Commands};
use systemprompt_cli::descriptor::{CommandDescriptor, DataImpact, DescribeCommand};
use systemprompt_cli::runner::profile_routing::require_explicit_cloud_profile;
use systemprompt_cli::shared::ProfileSource;
use systemprompt_identifiers::TenantId;
use systemprompt_manifest::Profile;
use systemprompt_manifest::profile::{CloudConfig, ProfileType, TrustedIssuer};
use systemprompt_test_fixtures::{ensure_test_bootstrap, test_db_pool};
use systemprompt_users::UserRepository;

fn trusted(issuer: &str, can_issue_id_jag: bool) -> TrustedIssuer {
    serde_json::from_value(serde_json::json!({
        "issuer": issuer,
        "jwks_uri": format!("{issuer}/jwks"),
        "audience": "aud",
        "can_issue_id_jag": can_issue_id_jag,
    }))
    .expect("trusted issuer")
}

fn descriptor(args: &[&str]) -> CommandDescriptor {
    let cli = Cli::try_parse_from(std::iter::once("systemprompt").chain(args.iter().copied()))
        .unwrap_or_else(|e| panic!("parse {args:?}: {e}"));
    cli.command
        .as_ref()
        .map_or(CommandDescriptor::FULL, Commands::descriptor)
}

fn cloud_profile() -> Profile {
    let boot = ensure_test_bootstrap();
    let yaml = std::fs::read_to_string(&boot.profile_path).expect("read the fixture profile");
    let mut profile: Profile = serde_yaml::from_str(&yaml).expect("parse the fixture profile");
    profile.name = "production".to_owned();
    profile.target = ProfileType::Cloud;
    profile.cloud = Some(CloudConfig {
        tenant_id: Some(TenantId::new("tenant_prod")),
        ..CloudConfig::default()
    });
    profile
}

#[test]
fn candidates_are_trusted_id_jag_issuers_other_than_our_own() {
    let candidates = id_jag_candidates(
        "https://self.example/",
        &[
            trusted("https://idp.example", true),
            trusted("https://plain.example", false),
            trusted("https://self.example", true),
        ],
    );
    assert_eq!(candidates, vec!["https://idp.example".to_owned()]);
}

#[test]
fn apply_never_infers_a_target_even_with_one_candidate() {
    let candidates = vec!["https://idp.example".to_owned()];
    let error = resolve_choice(&candidates, None, false, true).unwrap_err();
    assert!(error.to_string().contains("--to-issuer"), "{error:#}");
    assert_eq!(
        resolve_choice(&candidates, None, false, false).unwrap(),
        None
    );
}

#[test]
fn the_named_target_must_be_a_trusted_id_jag_issuer() {
    let candidates = vec!["https://idp.example".to_owned()];
    assert_eq!(
        resolve_choice(&candidates, Some("https://idp.example/"), false, true).unwrap(),
        Some(RekeyChoice::MoveTo("https://idp.example".to_owned()))
    );
    let error =
        resolve_choice(&candidates, Some("https://other.example"), false, true).unwrap_err();
    assert!(
        error.to_string().contains("not a trusted issuer"),
        "{error:#}"
    );
    assert_eq!(
        resolve_choice(&candidates, None, true, true).unwrap(),
        Some(RekeyChoice::Delete)
    );
    assert!(
        Cli::try_parse_from([
            "systemprompt",
            "admin",
            "users",
            "federated-rekey",
            "--to-issuer",
            "x",
            "--delete",
        ])
        .is_err()
    );
}

#[test]
fn rekey_is_destructive_and_refuses_an_implicit_cloud_profile() {
    for (args, impact) in [
        (
            vec!["admin", "users", "federated-rekey"],
            DataImpact::Destructive,
        ),
        (
            vec!["admin", "users", "federated-rekey", "--apply", "--delete"],
            DataImpact::Irreversible,
        ),
        (
            vec!["admin", "config", "secret", "migrate-git-credentials"],
            DataImpact::Destructive,
        ),
        (
            vec![
                "admin",
                "config",
                "secret",
                "migrate-git-credentials",
                "--apply",
            ],
            DataImpact::Destructive,
        ),
    ] {
        let desc = descriptor(&args);
        assert_eq!(desc.data_impact(), impact, "{args:?}");
        let profile = cloud_profile();
        for source in [ProfileSource::Session, ProfileSource::Discovery] {
            require_explicit_cloud_profile(&profile, source, &desc)
                .expect_err("an implicitly selected cloud profile must be refused");
        }
        require_explicit_cloud_profile(&profile, ProfileSource::Cli, &desc)
            .expect("an explicit --profile is accepted");
    }
}

#[test]
fn signing_key_fingerprint_preserves_data() {
    let desc = descriptor(&["admin", "bridge", "signing-key-fingerprint"]);
    assert_eq!(desc.data_impact(), DataImpact::Preserving);
    assert!(!desc.database());
}

async fn links_under(pool: &systemprompt_database::DbPool, issuer: &str) -> Vec<String> {
    let mut rows: Vec<(String,)> =
        sqlx::query_as("SELECT external_sub FROM federated_identities WHERE issuer = $1")
            .bind(issuer)
            .fetch_all(pool.pool().as_ref())
            .await
            .expect("read links");
    rows.sort();
    rows.into_iter().map(|(sub,)| sub).collect()
}

#[tokio::test]
async fn dry_run_reports_and_apply_moves_the_links() {
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let repo = UserRepository::new(&pool);
    let tag = uuid::Uuid::new_v4().simple().to_string();
    let own = format!("https://self-{tag}.invalid");
    let upstream = format!("https://upstream-{tag}.invalid");
    let user = repo
        .create(
            &format!("rekey-{tag}"),
            &format!("rekey-{tag}@x.invalid"),
            None,
            None,
        )
        .await
        .expect("create user");
    sqlx::query(
        "INSERT INTO federated_identities (issuer, external_sub, user_id) VALUES ($1, 'sub-1', $2)",
    )
    .bind(&own)
    .bind(user.id.as_str())
    .execute(pool.pool().as_ref())
    .await
    .expect("insert link");
    let candidates = vec![upstream.clone()];

    let dry = run(&repo, &own, &candidates, None, None)
        .await
        .expect("dry run");
    let shown: Vec<_> = dry
        .options
        .iter()
        .map(|o| (o.action.as_str(), o.target_issuer.clone(), o.rows))
        .collect();
    assert_eq!(
        shown,
        vec![("move", Some(upstream.clone()), 1), ("delete", None, 1)],
        "the dry run lists every choice and picks none"
    );
    assert_eq!((dry.applied, dry.moved, dry.removed), (None, 0, 0));
    assert_eq!(links_under(&pool, &own).await, vec!["sub-1"]);

    let choice = RekeyChoice::MoveTo(upstream.clone());
    let applied = run(&repo, &own, &candidates, Some(&choice), Some(&choice))
        .await
        .expect("apply");
    assert_eq!((applied.moved, applied.removed), (1, 0));
    assert!(links_under(&pool, &own).await.is_empty());
    assert_eq!(links_under(&pool, &upstream).await, vec!["sub-1"]);
}
