//! The routing plane: which command may bypass cloud routing, what a profile
//! that cannot reach its tenant is allowed to do, and what the failure advises.
//!
//! These decisions live in `systemprompt_cli::runner::routing` and
//! `systemprompt_cli::runner::profile_routing`. The read paths resolve against
//! the checkout's own `.systemprompt` directory, so they are driven only where
//! the outcome is a refusal — nothing here writes.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::args::Cli;
use systemprompt_cli::descriptor::RoutingClass;
use systemprompt_cli::runner::profile_routing::{
    BootstrapOutcome, RoutingDecision, allow_local_execution, confirm_remote_job_run,
    decide_routing, is_cloud_bypass_command, remediation_for,
};
use systemprompt_cli::runner::routing::{
    ExecutionTarget, determine_execution_target, execute_remote, load_session_for_key,
    resolve_tenant,
};
use systemprompt_cli::{CliConfig, OutputFormat};
use systemprompt_cloud::SessionKey;
use systemprompt_identifiers::{ContextId, SessionToken, TenantId};
use systemprompt_models::Profile;

fn cli(args: &[&str]) -> Cli {
    Cli::try_parse_from(std::iter::once("systemprompt").chain(args.iter().copied()))
        .unwrap_or_else(|e| panic!("parse {args:?}: {e}"))
}

fn fixture_profile() -> Profile {
    let boot = systemprompt_test_fixtures::ensure_test_bootstrap();
    let yaml = std::fs::read_to_string(&boot.profile_path).expect("read the fixture profile");
    serde_yaml::from_str(&yaml).expect("parse the fixture profile")
}

fn message(err: &anyhow::Error) -> String {
    format!("{err:#}")
}

#[test]
fn cloud_and_session_commands_bypass_cloud_routing_and_others_do_not() {
    assert!(is_cloud_bypass_command(
        cli(&["cloud", "doctor"]).command.as_ref()
    ));
    assert!(is_cloud_bypass_command(
        cli(&["admin", "session", "list"]).command.as_ref()
    ));
    assert!(!is_cloud_bypass_command(
        cli(&["infra", "services", "status"]).command.as_ref()
    ));
    assert!(!is_cloud_bypass_command(None));
}

#[test]
fn a_local_profile_routes_locally() {
    let _profile = fixture_profile();

    let target = determine_execution_target().expect("a local profile resolves a target");

    assert!(
        matches!(target, ExecutionTarget::Local),
        "a profile whose target is local must not route remotely, got {target:?}"
    );
}

#[test]
fn a_tenant_that_is_not_in_the_local_store_is_reported_with_the_sync_command() {
    let profile = fixture_profile();

    let err = resolve_tenant(&profile, &TenantId::new("tenant_that_was_never_synced"))
        .expect_err("an unsynced tenant cannot be resolved");

    let message = message(&err);
    assert!(
        message.contains("cloud tenant list"),
        "the failure must tell the operator how to sync, got: {message}"
    );
}

#[test]
fn a_key_with_no_stored_session_says_to_log_in() {
    let profile = fixture_profile();
    let key = SessionKey::Tenant(TenantId::new("tenant_with_no_session_at_all"));

    let err = load_session_for_key(&profile, &key, "http://localhost:8080")
        .expect_err("a key with no session cannot resolve one");

    assert!(
        message(&err).contains("admin session login"),
        "the failure must name the login command, got: {}",
        message(&err)
    );
}

// Why: `remediation_for` branches on the reason text rather than a type, so the
// two callers' wordings are pinned here — a reason reworded upstream silently
// starts advising a login for a tenant-store failure.
#[test]
fn a_tenant_failure_advises_syncing_and_anything_else_advises_signing_in() {
    assert!(
        remediation_for("no tenant is configured").contains("cloud tenant list"),
        "a tenant-shaped reason must advise a sync"
    );
    assert!(
        remediation_for("routing failed: could not load tenants from disk")
            .contains("cloud tenant list"),
        "a tenant-store failure must advise a sync, not a login"
    );
    assert!(
        remediation_for("routing failed: connection refused").contains("admin session login"),
        "any other reason must advise a login"
    );
}

#[test]
fn external_database_access_lets_a_mutating_command_run_locally() {
    let mut profile = fixture_profile();
    profile.database.external_db_access = true;

    allow_local_execution(&profile, RoutingClass::Mutating, "no tenant is configured")
        .expect("external database access is the deliberate escape hatch");
}

#[test]
fn a_read_only_command_falls_back_to_local_data_with_a_warning() {
    let mut profile = fixture_profile();
    profile.database.external_db_access = false;

    allow_local_execution(&profile, RoutingClass::ReadOnly, "no tenant is configured")
        .expect("a read may proceed against local data");
}

#[test]
fn a_mutating_command_refuses_rather_than_writing_to_the_wrong_database() {
    let mut profile = fixture_profile();
    profile.database.external_db_access = false;

    let err = allow_local_execution(&profile, RoutingClass::Mutating, "no tenant is configured")
        .expect_err("a mutation must not silently target the local database");

    let message = message(&err);
    assert!(
        message.contains(&profile.name) && message.contains("no tenant is configured"),
        "the refusal must name the profile and the reason, got: {message}"
    );
    assert!(
        message.contains("cloud tenant list"),
        "the refusal must carry the remediation for a tenant reason, got: {message}"
    );
}

#[test]
fn only_a_jobs_run_command_is_confirmed_before_it_reaches_a_remote_profile() {
    let config = CliConfig::new()
        .with_interactive(false)
        .with_output_format(OutputFormat::Json);

    confirm_remote_job_run(
        &cli(&["infra", "services", "status"]),
        &config,
        "prod",
        "example.invalid",
    )
    .expect("a command that is not a jobs run needs no confirmation");
}

#[test]
fn a_remote_jobs_run_is_refused_without_a_terminal_to_confirm_on() {
    let config = CliConfig::new()
        .with_interactive(false)
        .with_output_format(OutputFormat::Json);

    for args in [
        vec!["infra", "jobs", "run", "publish_pipeline"],
        vec!["infra", "jobs", "run", "--all"],
        vec!["infra", "jobs", "run", "--tag", "nightly"],
    ] {
        let err = confirm_remote_job_run(&cli(&args), &config, "prod", "example.invalid")
            .expect_err("an unconfirmable remote jobs run must not proceed");

        assert!(
            !message(&err).is_empty(),
            "{args:?} must refuse with a message"
        );
    }
}

#[test]
fn a_confirmed_jobs_run_passes_the_gate() {
    let config = CliConfig::new()
        .with_interactive(false)
        .with_output_format(OutputFormat::Json);

    confirm_remote_job_run(
        &cli(&["infra", "jobs", "run", "publish_pipeline", "--yes"]),
        &config,
        "prod",
        "example.invalid",
    )
    .expect("--yes is the non-interactive confirmation");
}

#[tokio::test]
async fn an_unreachable_host_is_a_connection_error_not_an_exit_code() {
    let context = ContextId::generate();
    let token = SessionToken::new("token-that-is-never-checked");

    // Why: a host that refuses the connection never produced a stream, so the
    // runner must see an error rather than an exit code the shell would trust.
    let err = execute_remote(
        "127.0.0.1:1",
        &token,
        &context,
        &[
            "infra".to_owned(),
            "services".to_owned(),
            "status".to_owned(),
        ],
        1,
    )
    .await
    .expect_err("a refused connection is not a remote exit code");

    assert!(!err.to_string().is_empty());
}

fn cloud_profile() -> Profile {
    let mut profile = fixture_profile();
    profile.target = systemprompt_models::profile::ProfileType::Cloud;
    profile.database.external_db_access = false;
    profile
}

// Why: a remote target must resolve to *one* execution. Reporting the remote
// run as "continue locally" made the runner dispatch the same mutating
// command a second time against whatever database the cloud profile resolves.
#[test]
fn a_remote_target_is_executed_remotely_and_nowhere_else() {
    let decision = decide_routing(
        Ok(ExecutionTarget::Remote {
            hostname: "tenant.example".to_owned(),
            token: SessionToken::new("tok"),
            context: ContextId::generate(),
        }),
        &cloud_profile(),
        RoutingClass::Mutating,
    )
    .expect("a resolved remote target is a decision, not an error");

    assert!(
        matches!(decision, RoutingDecision::ExecuteRemote { ref hostname, .. } if hostname == "tenant.example"),
        "{decision:?}"
    );
    assert_ne!(
        BootstrapOutcome::RemoteExecuted,
        BootstrapOutcome::ContinueLocal,
        "the runner must be able to tell a finished remote run from a local continuation"
    );
}

#[test]
fn a_cloud_profile_with_no_tenant_lets_a_read_only_command_continue_locally() {
    let decision = decide_routing(
        Ok(ExecutionTarget::Local),
        &cloud_profile(),
        RoutingClass::ReadOnly,
    )
    .expect("read-only work may fall back to local data");

    assert_eq!(decision, RoutingDecision::ContinueLocal);
}

#[test]
fn a_cloud_profile_that_cannot_route_refuses_a_mutating_command() {
    let err = decide_routing(
        Err(anyhow::anyhow!("no session")),
        &cloud_profile(),
        RoutingClass::Mutating,
    )
    .expect_err("a mutating command must not run against an unknown database");

    assert!(
        message(&err).contains("requires remote execution"),
        "{err:#}"
    );
}

#[test]
fn a_local_profile_continues_locally_whatever_the_target_says() {
    let decision = decide_routing(
        Err(anyhow::anyhow!("irrelevant")),
        &fixture_profile(),
        RoutingClass::Mutating,
    )
    .expect("a local profile never routes");

    assert_eq!(decision, RoutingDecision::ContinueLocal);
}
