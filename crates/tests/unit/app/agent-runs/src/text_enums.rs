//! Closed enums round-trip through their `TEXT` column spelling.

use std::str::FromStr;

use systemprompt_agent_runs::{ErrorClass, Execution, HostKind, RunStatus, host_for};

#[test]
fn host_kind_round_trips() {
    for kind in [
        HostKind::ClaudeCode,
        HostKind::CodexCli,
        HostKind::OpenCodeCli,
        HostKind::Hermes,
    ] {
        assert_eq!(HostKind::from_str(kind.as_str()).expect("parse"), kind);
        let json = serde_json::to_string(&kind).expect("serialize");
        assert_eq!(json, format!("\"{}\"", kind.as_str()));
    }
    assert!(HostKind::from_str("cursor").is_err());
}

#[test]
fn status_and_error_class_round_trip() {
    assert_eq!(
        RunStatus::from_str("timed_out").expect("parse"),
        RunStatus::TimedOut
    );
    assert!(RunStatus::TimedOut.is_terminal());
    assert!(!RunStatus::Running.is_terminal());
    assert_eq!(
        ErrorClass::from_str("hook_denied").expect("parse"),
        ErrorClass::HookDenied
    );
}

#[test]
fn undeclared_hosts_are_unsupported() {
    assert!(matches!(
        host_for(HostKind::OpenCodeCli),
        Execution::Unsupported { .. }
    ));
    for app in [HostKind::ClaudeDesktop, HostKind::Codex, HostKind::OpenCode] {
        assert!(
            matches!(host_for(app), Execution::Unsupported { .. }),
            "{app} is a desktop packaging and never runs headless"
        );
    }
    assert!(matches!(
        host_for(HostKind::Hermes),
        Execution::Unsupported { .. }
    ));
}
