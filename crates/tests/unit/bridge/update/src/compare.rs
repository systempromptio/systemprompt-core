use systemprompt_bridge::gateway::types::ReleaseManifest;
use systemprompt_bridge::update::{UpdateError, UpdateStatus, compare};

fn manifest(version: &str) -> ReleaseManifest {
    ReleaseManifest {
        version: version.to_owned(),
        sha256: "0".repeat(64),
        size: 1,
        notes_url: None,
    }
}

#[test]
fn newer_remote_is_available() {
    assert!(matches!(
        compare("0.1.6", &manifest("0.1.7")),
        Ok(UpdateStatus::Available { ref version, .. }) if version == "0.1.7"
    ));
}

#[test]
fn equal_version_is_current() {
    assert!(matches!(
        compare("0.1.6", &manifest("0.1.6")),
        Ok(UpdateStatus::Current { .. })
    ));
}

#[test]
fn local_ahead_of_gateway_is_current() {
    assert!(matches!(
        compare("0.2.0", &manifest("0.1.6")),
        Ok(UpdateStatus::Current { .. })
    ));
}

#[test]
fn patch_ten_is_newer_than_patch_nine() {
    assert!(matches!(
        compare("0.1.9", &manifest("0.1.10")),
        Ok(UpdateStatus::Available { .. })
    ));
}

#[test]
fn prerelease_is_older_than_its_release() {
    assert!(matches!(
        compare("0.1.7-rc.1", &manifest("0.1.7")),
        Ok(UpdateStatus::Available { .. })
    ));
}

#[test]
fn malformed_remote_version_is_an_error() {
    assert!(matches!(
        compare("0.1.6", &manifest("latest")),
        Err(UpdateError::BadRemoteVersion { .. })
    ));
}

#[test]
fn malformed_local_version_is_an_error() {
    assert!(matches!(
        compare("dev", &manifest("0.1.6")),
        Err(UpdateError::BadLocalVersion { .. })
    ));
}

#[test]
fn bad_remote_version_error_quotes_the_offending_string() {
    let err = compare("0.1.6", &manifest("not-a-version")).expect_err("unparseable remote");
    assert!(
        err.to_string().contains("\"not-a-version\""),
        "message did not name the version: {err}"
    );
}

#[test]
fn available_carries_the_manifest_notes_url() {
    let mut m = manifest("2.0.0");
    m.notes_url = Some("https://example.test/release/2.0.0".to_owned());
    let status = compare("1.9.0", &m).expect("comparable");
    match status {
        UpdateStatus::Available { version, notes_url } => {
            assert_eq!(version, "2.0.0");
            assert_eq!(
                notes_url.as_deref(),
                Some("https://example.test/release/2.0.0")
            );
        },
        other => panic!("expected Available, got {other:?}"),
    }
}

#[test]
fn current_reports_the_local_version_not_the_remote_one() {
    let status = compare("2.5.0", &manifest("2.4.0")).expect("comparable");
    assert_eq!(
        status,
        UpdateStatus::Current {
            version: "2.5.0".to_owned()
        }
    );
}

#[test]
fn a_newer_prerelease_does_not_supersede_the_release() {
    let status = compare("1.0.0", &manifest("1.0.1-rc.1")).expect("comparable");
    assert_eq!(
        status,
        UpdateStatus::Available {
            version: "1.0.1-rc.1".to_owned(),
            notes_url: None
        }
    );
    assert!(matches!(
        compare("1.0.0", &manifest("1.0.0-rc.1")),
        Ok(UpdateStatus::Current { .. })
    ));
}

#[test]
fn major_version_jump_is_available() {
    assert!(matches!(
        compare("1.9.9", &manifest("2.0.0")),
        Ok(UpdateStatus::Available { .. })
    ));
}
