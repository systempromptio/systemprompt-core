//! Tests for small public helpers that no other test names.
//!
//! Each is a pure projection or a file writer taking an explicit path, so they
//! are testable directly rather than through their commands.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::EnvOverrides;
use systemprompt_cli::admin::setup::SetupArgs;

#[derive(Debug, Parser)]
struct SetupHarness {
    #[command(flatten)]
    args: SetupArgs,
}

fn setup_args(extra: &[&str]) -> SetupArgs {
    SetupHarness::try_parse_from(std::iter::once("setup").chain(extra.iter().copied()))
        .unwrap()
        .args
}

#[test]
fn database_identifiers_default_to_the_environment_name() {
    let args = setup_args(&[]);

    assert_eq!(args.effective_db_user("staging"), "systemprompt_staging");
    assert_eq!(args.effective_db_name("staging"), "systemprompt_staging");
}

#[test]
fn explicit_database_identifiers_win_over_the_environment_default() {
    let args = setup_args(&["--db-user", "custom_user", "--db-name", "custom_db"]);

    assert_eq!(args.effective_db_user("staging"), "custom_user");
    assert_eq!(args.effective_db_name("staging"), "custom_db");
}

#[test]
fn process_environment_overrides_are_read_without_panicking() {
    let from_process = EnvOverrides::from_process_env();
    let from_vars = EnvOverrides::from_vars([
        ("SYSTEMPROMPT_PROFILE", "/tmp/cov-profile.yaml"),
        ("NO_COLOR", "1"),
    ]);

    assert_eq!(
        from_vars.profile.as_deref(),
        Some("/tmp/cov-profile.yaml"),
        "explicit vars are honoured"
    );
    assert!(from_vars.no_color);
    // The ambient environment decides these; only their agreement is assertable.
    assert_eq!(
        from_process.profile.is_some(),
        std::env::var("SYSTEMPROMPT_PROFILE").is_ok()
    );
}
