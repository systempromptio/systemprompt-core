use sp_protocol::host::{enables_host, entry_targets_host, rule_targets_host};
use sp_protocol::{HOST_HEADER_VALUE, HOST_ID};

#[test]
fn enabled_hosts_must_name_the_host() {
    assert!(enables_host(&["claude-code", "harness"], HOST_ID));
    assert!(!enables_host(&["claude-code"], HOST_ID));
    assert!(!enables_host::<&str>(&[], HOST_ID));
    assert!(!enables_host(&["harness-cli"], HOST_ID));
}

#[test]
fn entries_without_hosts_target_every_host() {
    assert!(entry_targets_host::<&str>(&[], HOST_ID));
    assert!(entry_targets_host(&["codex", "harness"], HOST_ID));
    assert!(!entry_targets_host(&["codex"], HOST_ID));
}

#[test]
fn rules_need_an_explicit_host() {
    assert!(!rule_targets_host::<&str>(&[], HOST_ID));
    assert!(rule_targets_host(&["harness"], HOST_ID));
    assert!(!rule_targets_host(&["claude-code"], HOST_ID));
}

#[test]
fn the_header_value_is_the_host_id() {
    assert_eq!(HOST_HEADER_VALUE, "harness");
    assert_eq!(HOST_ID, "harness");
}
