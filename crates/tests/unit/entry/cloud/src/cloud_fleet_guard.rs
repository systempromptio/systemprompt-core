//! `cloud fleet run` forwards read-only commands only: the guard classifies
//! the forwarded command with the router's own descriptor before any tenant
//! is contacted.

use systemprompt_cli::kernel::{
    CommandRegistry, command_line_path, describe_command_line, kernel_commands,
};
use systemprompt_cloud_entry::cli::fleet::ensure_read_only;
use systemprompt_extension::cli::DescribedCommand;

fn args(command: &[&str]) -> Vec<String> {
    command.iter().map(|arg| (*arg).to_owned()).collect()
}

fn guard(command: &[String]) -> anyhow::Result<()> {
    let declared = systemprompt::reference()
        .declare()
        .expect("the reference composition declares");
    let registry = CommandRegistry::build(&declared.commands(), &kernel_commands())
        .expect("the registry accepts the reference commands");
    let described = DescribedCommand {
        path: command_line_path(&registry, command)?,
        descriptor: describe_command_line(&registry, command)?,
    };
    ensure_read_only(&described, command)
}

#[test]
fn a_read_only_log_query_is_forwarded() {
    guard(&args(&["infra", "logs", "summary"])).expect("logs summary is read-only");
}

#[test]
fn a_destructive_command_is_refused() {
    let err = guard(&args(&["infra", "db", "migrate"])).unwrap_err();
    assert!(
        err.to_string().contains("not a read-only command"),
        "{err:#}"
    );
}

#[test]
fn a_mutating_command_is_refused() {
    let err = guard(&args(&["infra", "jobs", "list"])).unwrap_err();
    assert!(
        err.to_string().contains("not a read-only command"),
        "{err:#}"
    );
}

#[test]
fn a_local_only_command_is_refused() {
    let err = guard(&args(&["infra", "services", "status"])).unwrap_err();
    assert!(
        err.to_string().contains("not a read-only command"),
        "{err:#}"
    );
}

#[test]
fn cloud_commands_are_refused() {
    let err = guard(&args(&["cloud", "status"])).unwrap_err();
    assert!(err.to_string().contains("run on this machine"), "{err:#}");
}

#[test]
fn an_unknown_command_is_refused() {
    let err = guard(&args(&["no-such-command"])).unwrap_err();
    assert!(
        err.to_string().contains("is not a systemprompt command"),
        "{err:#}"
    );
}

#[test]
fn an_empty_command_is_refused() {
    assert!(ensure_read_only(&[]).is_err());
}

#[test]
fn irreversible_commands_are_kept_out_of_fleet_run() {
    for command in [
        &["infra", "db", "migrate-down", "users", "1"][..],
        &["infra", "db", "assign-admin", "u"],
        &["admin", "keys", "rotate", "--revoke"],
        &["admin", "users", "role", "promote", "u"],
    ] {
        let err = guard(&args(command)).unwrap_err();
        assert!(
            err.to_string().contains("not a read-only command"),
            "{command:?}: {err:#}"
        );
    }
}

#[test]
fn session_commands_are_refused_as_local_commands_not_as_cloud_commands() {
    for command in [
        &["admin", "session", "list"][..],
        &["admin", "session", "show"],
    ] {
        let err = guard(&args(command)).unwrap_err();
        let message = err.to_string();
        assert!(
            message.contains("not a read-only command"),
            "{command:?}: {err:#}"
        );
        assert!(!message.contains("cloud commands"), "{command:?}: {err:#}");
    }
}

#[test]
fn cloud_commands_behind_global_flags_are_refused_as_cloud_commands() {
    let err = guard(&args(&["--json", "cloud", "status"])).unwrap_err();
    assert!(err.to_string().contains("run on this machine"), "{err:#}");
}
