//! Tests for the database-scope refusal's reachable arms.
//!
//! The helper is called by the command tests only along its happy path; the
//! arms here are the ones a command hits when the environment is wrong.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use crate::boot_ctx::bootstrap_ctx;
use systemprompt_cli::shared::database_scoped_command_error;

#[test]
fn the_database_scope_error_names_the_active_profile_once_one_is_installed() {
    let rendered = database_scoped_command_error(&bootstrap_ctx()).to_string();

    assert!(
        rendered.contains("routes to an external/cloud database"),
        "{rendered}"
    );
    assert!(rendered.contains("--profile <local-profile>"), "{rendered}");
}

#[test]
fn the_database_scope_error_without_a_profile_asks_for_one() {
    let bare = systemprompt_cli::CommandContext::new(
        systemprompt_extension::cli::CliConfig::new().with_interactive(false),
        systemprompt_cli::EnvOverrides::default(),
    );

    let rendered = database_scoped_command_error(&bare).to_string();

    assert!(
        rendered.contains("requires full profile context"),
        "{rendered}"
    );
}
