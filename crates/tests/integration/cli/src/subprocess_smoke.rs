//! Subprocess tests that invoke the `systemprompt` binary via `assert_cmd`.
//!
//! These exist so that the cli's instrumented binary actually executes under
//! `just coverage` and contributes line counts. Each `Command::cargo_bin`
//! invocation inherits `LLVM_PROFILE_FILE` from the parent test process; the
//! child writes its own profraw which the merge step folds into the report.

use assert_cmd::Command;
use predicates::str::contains;

#[test]
fn version_flag_prints_crate_version() {
    Command::cargo_bin("systemprompt")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(contains("systemprompt"));
}

#[test]
fn help_flag_lists_top_level_commands() {
    Command::cargo_bin("systemprompt")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(contains("admin"))
        .stdout(contains("cloud"))
        .stdout(contains("infra"));
}

#[test]
fn unknown_subcommand_exits_nonzero() {
    Command::cargo_bin("systemprompt")
        .unwrap()
        .arg("definitely-not-a-real-command")
        .assert()
        .failure();
}

#[test]
fn admin_help_describes_subcommands() {
    Command::cargo_bin("systemprompt")
        .unwrap()
        .args(["admin", "--help"])
        .assert()
        .success();
}

#[test]
fn cloud_help_describes_subcommands() {
    Command::cargo_bin("systemprompt")
        .unwrap()
        .args(["cloud", "--help"])
        .assert()
        .success();
}

#[test]
fn infra_help_describes_subcommands() {
    Command::cargo_bin("systemprompt")
        .unwrap()
        .args(["infra", "--help"])
        .assert()
        .success();
}
