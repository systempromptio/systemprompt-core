use systemprompt_device::protocol::targets::{enables_host, entry_targets_host, rule_targets_host};
use systemprompt_models::bridge::host::HostKind;

#[test]
fn enabled_hosts_must_name_the_host() {
    assert!(enables_host(
        &[HostKind::ClaudeCode, HostKind::Harness],
        HostKind::Harness
    ));
    assert!(!enables_host(&[HostKind::ClaudeCode], HostKind::Harness));
    assert!(!enables_host(&[], HostKind::Harness));
}

#[test]
fn entries_without_hosts_target_every_host() {
    assert!(entry_targets_host::<&str>(&[], HostKind::Harness));
    assert!(entry_targets_host(&["codex", "harness"], HostKind::Harness));
    assert!(!entry_targets_host(&["codex"], HostKind::Harness));
}

#[test]
fn rules_need_an_explicit_host() {
    assert!(!rule_targets_host::<&str>(&[], HostKind::Harness));
    assert!(rule_targets_host(&["harness"], HostKind::Harness));
    assert!(!rule_targets_host(&["claude-code"], HostKind::Harness));
}

#[test]
fn the_header_value_is_the_host_id() {
    assert_eq!(HostKind::Harness.as_str(), "harness");
}
