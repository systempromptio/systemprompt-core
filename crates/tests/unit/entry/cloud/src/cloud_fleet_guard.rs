//! `cloud fleet run` forwards read-only commands only: the guard classifies
//! the forwarded command with the router's own descriptor before any tenant
//! is contacted.

use systemprompt_cli::cloud::fleet::ensure_read_only;

fn args(command: &[&str]) -> Vec<String> {
    command.iter().map(|arg| (*arg).to_owned()).collect()
}

#[test]
fn a_read_only_log_query_is_forwarded() {
    ensure_read_only(&args(&["infra", "logs", "summary"])).expect("logs summary is read-only");
}

#[test]
fn analytics_is_forwarded() {
    ensure_read_only(&args(&["analytics", "overview"])).expect("analytics is read-only");
}

#[test]
fn a_destructive_command_is_refused() {
    let err = ensure_read_only(&args(&["infra", "db", "migrate"])).unwrap_err();
    assert!(
        err.to_string().contains("not a read-only command"),
        "{err:#}"
    );
}

#[test]
fn a_mutating_command_is_refused() {
    let err = ensure_read_only(&args(&["infra", "jobs", "list"])).unwrap_err();
    assert!(
        err.to_string().contains("not a read-only command"),
        "{err:#}"
    );
}

#[test]
fn a_local_only_command_is_refused() {
    let err = ensure_read_only(&args(&["infra", "services", "status"])).unwrap_err();
    assert!(
        err.to_string().contains("not a read-only command"),
        "{err:#}"
    );
}

#[test]
fn cloud_commands_are_refused() {
    let err = ensure_read_only(&args(&["cloud", "status"])).unwrap_err();
    assert!(err.to_string().contains("run on this machine"), "{err:#}");
}

#[test]
fn an_unknown_command_is_refused() {
    let err = ensure_read_only(&args(&["no-such-command"])).unwrap_err();
    assert!(
        err.to_string().contains("is not a systemprompt command"),
        "{err:#}"
    );
}

#[test]
fn an_empty_command_is_refused() {
    assert!(ensure_read_only(&[]).is_err());
}
