//! An explicit `--profile` is a one-shot target: it never rewrites the active
//! session, and a cloud profile that arrived implicitly is refused by the
//! commands that mutate whatever database the profile resolves to.
//!
//! Both invariants exist because `just deploy-check --profile production`
//! once left the session index pointing at production and the next bare
//! `infra db migrate` nearly migrated it.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::args::{Cli, Commands};
use systemprompt_cli::descriptor::{CommandDescriptor, DescribeCommand};
use systemprompt_cli::runner::profile_routing::require_explicit_cloud_profile;
use systemprompt_cli::session::resolution::record_new_session;
use systemprompt_cli::shared::{ProfileSource, resolve_profile_path};
use systemprompt_cloud::{CliSession, SessionBinding, SessionIdentity, SessionKey, SessionStore};
use systemprompt_identifiers::{
    ContextId, Email, ProfileName, SessionId, SessionToken, TenantId, UserId,
};
use systemprompt_models::Profile;
use systemprompt_models::auth::UserType;
use systemprompt_models::profile::{CloudConfig, ProfileType};
use tempfile::TempDir;

fn session(profile_name: &str) -> CliSession {
    CliSession::builder(
        SessionBinding::new(
            ProfileName::new(profile_name),
            "http://localhost:8080".to_owned(),
        ),
        SessionToken::new("tok"),
        SessionId::generate(),
        ContextId::generate(),
        SessionIdentity::new(
            UserId::new("user-explicit-profile"),
            Email::new("ops@example.test"),
            UserType::Admin,
        ),
    )
    .build()
}

fn fixture_profile() -> Profile {
    let boot = systemprompt_test_fixtures::ensure_test_bootstrap();
    let yaml = std::fs::read_to_string(&boot.profile_path).expect("read the fixture profile");
    serde_yaml::from_str(&yaml).expect("parse the fixture profile")
}

fn cloud_profile(name: &str) -> Profile {
    let mut profile = fixture_profile();
    profile.name = name.to_owned();
    profile.target = ProfileType::Cloud;
    profile.cloud = Some(CloudConfig {
        tenant_id: Some(TenantId::new("tenant_prod")),
        ..CloudConfig::default()
    });
    profile
}

fn descriptor(args: &[&str]) -> CommandDescriptor {
    let cli = Cli::try_parse_from(std::iter::once("systemprompt").chain(args.iter().copied()))
        .unwrap_or_else(|e| panic!("parse {args:?}: {e}"));
    cli.command
        .as_ref()
        .map_or(CommandDescriptor::FULL, Commands::descriptor)
}

fn store_with_active_local(dir: &TempDir) -> SessionStore {
    let mut store = SessionStore::load_or_create(dir.path()).expect("fresh store");
    store.upsert_session(&SessionKey::Local, session("local"));
    store.set_active_with_profile(&SessionKey::Local, "local");
    store
}

#[test]
fn resolve_profile_path_reports_where_the_profile_came_from() {
    let dir = TempDir::new().expect("tempdir");
    let yaml = dir.path().join("profile.yaml");
    std::fs::write(&yaml, "name: t\n").expect("write");
    let path = yaml.to_str().expect("utf8");

    assert_eq!(
        resolve_profile_path(Some(path), None, None)
            .expect("cli")
            .source,
        ProfileSource::Cli
    );
    assert_eq!(
        resolve_profile_path(None, Some(path), None)
            .expect("env")
            .source,
        ProfileSource::Env
    );
    assert_eq!(
        resolve_profile_path(None, None, Some(yaml.clone()))
            .expect("session")
            .source,
        ProfileSource::Session
    );
    assert!(ProfileSource::Cli.is_explicit());
    assert!(ProfileSource::Env.is_explicit());
    assert!(!ProfileSource::Session.is_explicit());
    assert!(!ProfileSource::Discovery.is_explicit());
}

#[test]
fn an_explicit_profile_override_stores_the_session_but_leaves_the_active_key_alone() {
    let dir = TempDir::new().expect("tempdir");
    let tenant_key = SessionKey::Tenant(TenantId::new("tenant_prod"));

    for source in [
        ProfileSource::Cli,
        ProfileSource::Env,
        ProfileSource::Discovery,
    ] {
        let mut store = store_with_active_local(&dir);

        record_new_session(
            &mut store,
            &tenant_key,
            &session("production"),
            "production",
            source,
        );

        assert!(
            store.get_session(&tenant_key).is_some(),
            "{source:?}: the minted session must be reusable next time"
        );
        assert_eq!(
            store.active_key.as_deref(),
            Some("local"),
            "{source:?}: a one-shot profile must not become the active session"
        );
        assert_eq!(store.active_profile_name.as_deref(), Some("local"));
    }
}

#[test]
fn a_session_selected_profile_still_becomes_active() {
    let dir = TempDir::new().expect("tempdir");
    let mut store = store_with_active_local(&dir);
    let tenant_key = SessionKey::Tenant(TenantId::new("tenant_prod"));

    record_new_session(
        &mut store,
        &tenant_key,
        &session("production"),
        "production",
        ProfileSource::Session,
    );

    assert_eq!(store.active_key.as_deref(), Some("tenant_tenant_prod"));
    assert_eq!(store.active_profile_name.as_deref(), Some("production"));
}

#[test]
fn migrate_refuses_an_implicit_cloud_profile_and_accepts_an_explicit_one() {
    let profile = cloud_profile("production");
    let migrate = descriptor(&["infra", "db", "migrate"]);
    assert!(migrate.requires_explicit_cloud_profile());

    for source in [ProfileSource::Session, ProfileSource::Discovery] {
        let err = require_explicit_cloud_profile(&profile, source, &migrate)
            .expect_err("an implicitly selected cloud profile must be refused");
        let message = format!("{err:#}");
        assert!(message.contains("`production`"), "{message}");
        assert!(message.contains("--profile production"), "{message}");
    }

    for source in [ProfileSource::Cli, ProfileSource::Env] {
        require_explicit_cloud_profile(&profile, source, &migrate)
            .expect("an explicit cloud profile is the operator's decision");
    }
}

#[test]
fn every_mutating_database_command_demands_an_explicit_cloud_profile() {
    for args in [
        vec!["infra", "db", "migrate"],
        vec!["infra", "db", "migrate-down", "users", "1"],
        vec!["infra", "db", "migrate-repair"],
        vec![
            "infra",
            "db",
            "migrate-mark-applied",
            "--extension",
            "users",
            "--version",
            "1",
        ],
        vec!["infra", "db", "execute", "DELETE FROM users"],
        vec!["infra", "db", "assign-admin", "u"],
        vec!["infra", "jobs", "run", "publish_pipeline"],
    ] {
        assert!(
            descriptor(&args).requires_explicit_cloud_profile(),
            "{args:?} mutates the resolved database and must carry the flag"
        );
    }

    for args in [
        vec!["infra", "db", "migrate-status"],
        vec!["infra", "db", "tables"],
        vec!["infra", "jobs", "list"],
        vec!["infra", "logs", "view"],
    ] {
        assert!(
            !descriptor(&args).requires_explicit_cloud_profile(),
            "{args:?} is read-only and must stay usable on the active profile"
        );
    }
}

#[test]
fn local_profiles_are_unaffected_by_the_explicit_cloud_rule() {
    let profile = fixture_profile();
    assert!(!profile.target.is_cloud());
    let migrate = descriptor(&["infra", "db", "migrate"]);

    for source in [
        ProfileSource::Session,
        ProfileSource::Discovery,
        ProfileSource::Cli,
        ProfileSource::Env,
    ] {
        require_explicit_cloud_profile(&profile, source, &migrate)
            .unwrap_or_else(|e| panic!("{source:?}: a local profile is never refused: {e:#}"));
    }
}

#[test]
fn a_cloud_profile_without_a_tenant_is_still_a_cloud_target() {
    let mut profile = cloud_profile("staging");
    profile.cloud = None;
    let migrate = descriptor(&["infra", "db", "migrate"]);

    require_explicit_cloud_profile(&profile, ProfileSource::Session, &migrate)
        .expect_err("target: cloud alone is enough to demand --profile");
}
