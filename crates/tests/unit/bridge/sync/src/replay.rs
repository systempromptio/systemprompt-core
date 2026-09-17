use std::fs;

use systemprompt_bridge::gateway::manifest_version::ManifestVersion;
use systemprompt_bridge::sync::{
    LastSyncState, ReplayStateError, SyncError, check_replay, check_skew, read_last_sync,
};

fn tempdir() -> std::path::PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!(
        "bridge-replay-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&p).unwrap();
    p
}

fn version(s: &str) -> ManifestVersion {
    ManifestVersion::try_new(s).expect("valid manifest version literal")
}

fn last(v: &str) -> LastSyncState {
    LastSyncState {
        manifest_version: Some(version(v)),
        ..LastSyncState::default()
    }
}

#[test]
fn stale_replay_same_version_rejected() {
    let s = last("2026-04-22T10:00:00Z-abcdef01");
    let err =
        check_replay(&s, &version("2026-04-22T10:00:00Z-abcdef01")).expect_err("expected reject");
    assert!(matches!(err, SyncError::ReplayedManifest { .. }));
}

#[test]
fn older_version_rejected() {
    let s = last("2026-04-22T10:00:00Z-abcdef01");
    let err =
        check_replay(&s, &version("2026-04-21T09:00:00Z-fffffff0")).expect_err("expected reject");
    assert!(matches!(err, SyncError::ReplayedManifest { .. }));
}

#[test]
fn newer_version_accepted() {
    let s = last("2026-04-22T10:00:00Z-abcdef01");
    check_replay(&s, &version("2026-04-22T10:00:01Z-abcdef01")).expect("newer version should pass");
}

#[test]
fn no_prior_state_accepted() {
    let s = LastSyncState::default();
    check_replay(&s, &version("2026-04-22T10:00:00Z-abcdef01")).expect("first sync should pass");
}

#[test]
fn suffix_breaks_tie_when_timestamp_equal() {
    let s = last("2026-04-22T10:00:00Z-aaaaaaaa");
    check_replay(&s, &version("2026-04-22T10:00:00Z-bbbbbbbb"))
        .expect("higher hex suffix at same timestamp should pass");
    let err = check_replay(&s, &version("2026-04-22T10:00:00Z-00000000"))
        .expect_err("lower suffix should reject");
    assert!(matches!(err, SyncError::ReplayedManifest { .. }));
}

#[test]
fn manifest_version_rejects_missing_separator() {
    let err = ManifestVersion::try_new("no-separator-but-no-rfc3339").expect_err("must reject");
    let _ = format!("{err}");
}

#[test]
fn manifest_version_rejects_non_hex_suffix() {
    let err = ManifestVersion::try_new("2026-04-22T10:00:00Z-NOTHEXES")
        .expect_err("non-hex suffix must reject");
    let _ = format!("{err}");
}

#[test]
fn manifest_version_rejects_short_suffix() {
    let err =
        ManifestVersion::try_new("2026-04-22T10:00:00Z-abc").expect_err("short suffix must reject");
    let _ = format!("{err}");
}

#[test]
fn manifest_version_rejects_bad_timestamp() {
    let err =
        ManifestVersion::try_new("nope-abcdef01").expect_err("non-rfc3339 prefix must reject");
    let _ = format!("{err}");
}

#[test]
fn not_before_ten_minutes_in_past_rejected() {
    let now = chrono::Utc::now();
    let nb = now - chrono::Duration::minutes(10);
    let err = check_skew(nb, now).expect_err("10m past should reject");
    assert!(matches!(err, SyncError::ManifestSkew { .. }));
}

#[test]
fn not_before_ten_minutes_in_future_rejected() {
    let now = chrono::Utc::now();
    let nb = now + chrono::Duration::minutes(10);
    let err = check_skew(nb, now).expect_err("10m future should reject");
    assert!(matches!(err, SyncError::ManifestSkew { .. }));
}

#[test]
fn not_before_thirty_seconds_past_accepted() {
    let now = chrono::Utc::now();
    let nb = now - chrono::Duration::seconds(30);
    check_skew(nb, now).expect("30s past should pass");
}

#[test]
fn not_before_thirty_seconds_future_accepted() {
    let now = chrono::Utc::now();
    let nb = now + chrono::Duration::seconds(30);
    check_skew(nb, now).expect("30s future should pass");
}

#[test]
fn force_replay_bypasses_replay_and_skew() {
    let s = last("2026-04-22T10:00:00Z-abcdef01");
    assert!(check_replay(&s, &version("2026-04-21T09:00:00Z-aaaaaaaa")).is_err());

    let now = chrono::Utc::now();
    let nb = now - chrono::Duration::minutes(30);
    assert!(check_skew(nb, now).is_err());
}

#[test]
fn read_last_sync_reads_new_field() {
    let dir = tempdir();
    let path = dir.join("last-sync.json");
    fs::write(
        &path,
        r#"{"manifest_version":"2026-04-22T10:00:00Z-abcdef01"}"#,
    )
    .unwrap();
    let s = read_last_sync(&path).expect("valid file").expect("found");
    assert_eq!(
        s.manifest_version
            .as_ref()
            .map(ToString::to_string)
            .as_deref(),
        Some("2026-04-22T10:00:00Z-abcdef01")
    );
}

#[test]
fn read_last_sync_missing_file_yields_none() {
    let dir = tempdir();
    let s = read_last_sync(&dir.join("nope.json")).expect("missing file is Ok(None)");
    assert!(s.is_none());
}

#[test]
fn read_last_sync_corrupt_file_propagates() {
    let dir = tempdir();
    let path = dir.join("corrupt.json");
    fs::write(&path, b"{ this is not json").unwrap();
    let err = read_last_sync(&path).expect_err("corrupt file must fail");
    assert!(matches!(err, ReplayStateError::Parse { .. }));
}

#[test]
fn read_last_sync_invalid_version_format_propagates() {
    let dir = tempdir();
    let path = dir.join("bad-version.json");
    fs::write(&path, r#"{"manifest_version":"not-a-valid-version"}"#).unwrap();
    let err = read_last_sync(&path).expect_err("invalid version must fail");
    assert!(matches!(err, ReplayStateError::Parse { .. }));
}

// Why: manifest versions are per gateway; the previous gateway's version must
// not make the new gateway's first manifest look like a replay.
#[test]
fn a_sentinel_from_another_gateway_does_not_belong_to_this_one() {
    let this = systemprompt_identifiers::ValidatedUrl::try_new("https://gw.example.com")
        .expect("valid ValidatedUrl");
    let other = systemprompt_identifiers::ValidatedUrl::try_new("http://localhost:8080")
        .expect("valid ValidatedUrl");
    let stamped = LastSyncState {
        gateway: Some(other),
        ..last("2026-04-22T10:00:00Z-abcdef01")
    };
    assert!(!stamped.belongs_to(&this));
    let mine = LastSyncState {
        gateway: Some(this.clone()),
        ..last("2026-04-22T10:00:00Z-abcdef01")
    };
    assert!(mine.belongs_to(&this));
    assert!(
        !last("2026-04-22T10:00:00Z-abcdef01").belongs_to(&this),
        "an unstamped sentinel cannot prove which gateway wrote it"
    );
}

#[test]
fn a_partial_sync_record_keeps_the_prior_checkpoint_so_the_retry_is_not_a_replay() {
    let prior = version("2026-09-17T11:04:01Z-000001a0af09abc9");
    let partial = version("2026-09-17T11:05:24Z-000001a0af0aee88");
    let recorded = LastSyncState {
        manifest_version: Some(prior.clone()),
        host_failures: vec!["claude-code: apply: EOF while parsing a value".to_owned()],
        ..LastSyncState::default()
    };

    assert!(recorded.is_partial());
    check_replay(&recorded, &partial).expect("the partially applied manifest can be retried");
    let err = check_replay(&recorded, &prior).expect_err("the prior manifest is still a replay");
    assert!(matches!(err, SyncError::ReplayedManifest { .. }));
}

#[test]
fn a_host_failure_folds_to_one_sentinel_line() {
    let failure = systemprompt_bridge::sync::HostFailure {
        host_id: systemprompt_bridge::ids::HostId::new("claude-code"),
        emitter: "apply".to_owned(),
        error: "io error in remove managed MCP policy: EOF\nsecond line".to_owned(),
        needs_elevation: false,
    };

    assert_eq!(
        failure.sentinel_line(),
        "claude-code: apply: io error in remove managed MCP policy: EOF"
    );
}

#[test]
fn a_partial_sync_record_keeps_the_prior_hosts_and_update_policy_not_the_half_applied_ones() {
    use systemprompt_bridge::gateway::manifest::AutoUpdatePolicy;

    let prior = LastSyncState {
        manifest_version: Some(version("2026-09-17T11:04:01Z-000001a0af09abc9")),
        enabled_hosts: vec!["claude-code".to_owned()],
        host_model_protocols: [("claude-code".to_owned(), vec!["anthropic".to_owned()])]
            .into_iter()
            .collect(),
        auto_update: AutoUpdatePolicy::default(),
        ..LastSyncState::default()
    };
    let attempted = LastSyncState {
        manifest_version: Some(version("2026-09-17T11:05:24Z-000001a0af0aee88")),
        enabled_hosts: vec!["claude-code".to_owned(), "cowork".to_owned()],
        host_model_protocols: [("cowork".to_owned(), vec!["openai".to_owned()])]
            .into_iter()
            .collect(),
        auto_update: AutoUpdatePolicy::Disabled,
        installed_plugins: vec!["kit".to_owned()],
        host_failures: vec!["cowork: apply: permission denied".to_owned()],
        ..LastSyncState::default()
    };

    let recorded = attempted.retaining_delivered_policy_of(&prior);

    assert!(recorded.is_partial());
    assert_eq!(recorded.manifest_version, prior.manifest_version);
    assert_eq!(
        recorded.enabled_hosts, prior.enabled_hosts,
        "a host whose emitter failed is not enabled"
    );
    assert_eq!(recorded.host_model_protocols, prior.host_model_protocols);
    assert_eq!(
        recorded.auto_update, prior.auto_update,
        "a policy from a manifest that did not apply is not in force"
    );
    assert_eq!(
        recorded.installed_plugins,
        vec!["kit".to_owned()],
        "what did land is still recorded"
    );
}
