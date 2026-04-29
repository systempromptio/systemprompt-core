use std::fs;

use systemprompt_cowork::gateway::manifest::{SignedManifest, UserId, canonical_payload};
use systemprompt_cowork::gateway::manifest_version::ManifestVersion;
use systemprompt_cowork::ids::ManifestSignature;
use systemprompt_cowork::sync::{
    LastSyncState, ReplayStateError, SyncError, check_replay, check_skew, read_last_sync,
};

fn tempdir() -> std::path::PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!(
        "cowork-replay-{}-{}",
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
        last_applied_manifest_version: Some(version(v)),
    }
}

#[test]
fn canonical_payload_includes_not_before_in_position() {
    let m = SignedManifest {
        manifest_version: version("2026-04-27T12:00:00Z-cafebabe"),
        issued_at: "2026-04-27T12:00:00+00:00".into(),
        not_before: "2026-04-27T12:00:00+00:00".into(),
        user_id: UserId::new("u1"),
        tenant_id: None,
        user: None,
        plugins: vec![],
        skills: vec![],
        agents: vec![],
        managed_mcp_servers: vec![],
        revocations: vec![],
        signature: ManifestSignature::new("ignored"),
    };
    let p = canonical_payload(&m).unwrap();
    assert!(p.contains(r#""not_before":"2026-04-27T12:00:00+00:00""#));
    let nb_pos = p.find(r#""not_before""#).unwrap();
    let uid_pos = p.find(r#""user_id""#).unwrap();
    let issued_pos = p.find(r#""issued_at""#).unwrap();
    assert!(issued_pos < nb_pos && nb_pos < uid_pos);
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
    let nb = (now - chrono::Duration::minutes(10)).to_rfc3339();
    let err = check_skew(&nb, now).expect_err("10m past should reject");
    assert!(matches!(err, SyncError::ManifestSkew { .. }));
}

#[test]
fn not_before_ten_minutes_in_future_rejected() {
    let now = chrono::Utc::now();
    let nb = (now + chrono::Duration::minutes(10)).to_rfc3339();
    let err = check_skew(&nb, now).expect_err("10m future should reject");
    assert!(matches!(err, SyncError::ManifestSkew { .. }));
}

#[test]
fn not_before_thirty_seconds_past_accepted() {
    let now = chrono::Utc::now();
    let nb = (now - chrono::Duration::seconds(30)).to_rfc3339();
    check_skew(&nb, now).expect("30s past should pass");
}

#[test]
fn not_before_thirty_seconds_future_accepted() {
    let now = chrono::Utc::now();
    let nb = (now + chrono::Duration::seconds(30)).to_rfc3339();
    check_skew(&nb, now).expect("30s future should pass");
}

#[test]
fn force_replay_bypasses_replay_and_skew() {
    let s = last("2026-04-22T10:00:00Z-abcdef01");
    assert!(check_replay(&s, &version("2026-04-21T09:00:00Z-aaaaaaaa")).is_err());

    let now = chrono::Utc::now();
    let nb = (now - chrono::Duration::minutes(30)).to_rfc3339();
    assert!(check_skew(&nb, now).is_err());
}

#[test]
fn read_last_sync_reads_new_field() {
    let dir = tempdir();
    let path = dir.join("last-sync.json");
    fs::write(
        &path,
        r#"{"last_applied_manifest_version":"2026-04-22T10:00:00Z-abcdef01"}"#,
    )
    .unwrap();
    let s = read_last_sync(&path).expect("valid file").expect("found");
    assert_eq!(
        s.last_applied_manifest_version
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
    fs::write(
        &path,
        r#"{"last_applied_manifest_version":"not-a-valid-version"}"#,
    )
    .unwrap();
    let err = read_last_sync(&path).expect_err("invalid version must fail");
    assert!(matches!(err, ReplayStateError::Parse { .. }));
}
