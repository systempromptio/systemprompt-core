use std::fs;

use systemprompt_cowork::gateway::manifest::{SignedManifest, canonical_payload};
use systemprompt_cowork::sync::{
    LastSyncState, SyncError, check_replay, check_skew, read_last_sync,
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

fn last(version: &str) -> LastSyncState {
    LastSyncState {
        last_applied_manifest_version: Some(version.to_string()),
    }
}

#[test]
fn canonical_payload_includes_not_before_in_position() {
    let m = SignedManifest {
        manifest_version: "2026-04-27T12:00:00Z-cafe".into(),
        issued_at: "2026-04-27T12:00:00+00:00".into(),
        not_before: "2026-04-27T12:00:00+00:00".into(),
        user_id: "u1".into(),
        tenant_id: None,
        user: None,
        plugins: vec![],
        skills: vec![],
        agents: vec![],
        managed_mcp_servers: vec![],
        revocations: vec![],
        signature: "ignored".into(),
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
    let s = last("2026-04-22T10:00:00Z-abcd");
    let err = check_replay(&s, "2026-04-22T10:00:00Z-abcd").expect_err("expected reject");
    assert!(matches!(err, SyncError::ReplayedManifest { .. }));
}

#[test]
fn older_version_rejected() {
    let s = last("2026-04-22T10:00:00Z-abcd");
    let err = check_replay(&s, "2026-04-21T09:00:00Z-zzzz").expect_err("expected reject");
    assert!(matches!(err, SyncError::ReplayedManifest { .. }));
}

#[test]
fn newer_version_accepted() {
    let s = last("2026-04-22T10:00:00Z-abcd");
    check_replay(&s, "2026-04-22T10:00:01Z-abcd").expect("newer version should pass");
}

#[test]
fn no_prior_state_accepted() {
    let s = LastSyncState::default();
    check_replay(&s, "2026-04-22T10:00:00Z-abcd").expect("first sync should pass");
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
    let s = last("2026-04-22T10:00:00Z-abcd");
    assert!(check_replay(&s, "2026-04-21T09:00:00Z-aaaa").is_err());

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
        r#"{"last_applied_manifest_version":"2026-04-22T10:00:00Z-abcd"}"#,
    )
    .unwrap();
    let s = read_last_sync(&path);
    assert_eq!(
        s.last_applied_manifest_version.as_deref(),
        Some("2026-04-22T10:00:00Z-abcd")
    );
}

#[test]
fn read_last_sync_missing_file_yields_default() {
    let dir = tempdir();
    let s = read_last_sync(&dir.join("nope.json"));
    assert!(s.last_applied_manifest_version.is_none());
}
