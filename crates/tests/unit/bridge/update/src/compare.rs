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
