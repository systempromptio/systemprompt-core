use std::path::PathBuf;

use systemprompt_bridge::update::UpdateError;

#[test]
fn unsupported_platform_names_the_absent_build() {
    assert_eq!(
        UpdateError::UnsupportedPlatform.to_string(),
        "this platform has no published build"
    );
}

#[test]
fn checksum_mismatch_reports_both_digests() {
    let message = UpdateError::ChecksumMismatch {
        expected: "aaaa".to_owned(),
        actual: "bbbb".to_owned(),
    }
    .to_string();
    assert_eq!(message, "checksum mismatch: expected aaaa, got bbbb");
}

#[test]
fn download_status_reports_the_rejecting_code() {
    let message = UpdateError::DownloadStatus {
        status: reqwest::StatusCode::FORBIDDEN,
    }
    .to_string();
    assert_eq!(
        message,
        "gateway rejected the download: status=403 Forbidden"
    );
}

#[test]
fn not_writable_carries_the_remediation_hint() {
    let message = UpdateError::NotWritable {
        path: PathBuf::from("/usr/local/bin/bridge"),
        hint: "re-run with sudo".to_owned(),
    }
    .to_string();
    assert_eq!(
        message,
        "/usr/local/bin/bridge is not writable; re-run with sudo"
    );
}

#[test]
fn locate_install_names_what_was_missing() {
    let message = UpdateError::LocateInstall {
        what: "executable",
        detail: "no such file".to_owned(),
    }
    .to_string();
    assert_eq!(
        message,
        "could not locate the running executable: no such file"
    );
}

#[test]
fn no_staging_dir_and_unpack_and_signature_render() {
    assert_eq!(
        UpdateError::NoStagingDir.to_string(),
        "could not resolve the staging directory"
    );
    assert_eq!(
        UpdateError::Unpack("tar failed".to_owned()).to_string(),
        "unpacking the download failed: tar failed"
    );
    assert_eq!(
        UpdateError::Signature("bad cosign bundle".to_owned()).to_string(),
        "signature verification failed: bad cosign bundle"
    );
}

#[test]
fn version_changed_reports_both_versions() {
    let message = UpdateError::VersionChanged {
        expected: "1.0.0".to_owned(),
        actual: "1.0.1".to_owned(),
    }
    .to_string();
    assert_eq!(
        message,
        "the published version changed from 1.0.0 to 1.0.1 mid-install; try again"
    );
}

#[test]
fn relaunch_error_wraps_the_io_cause() {
    let message = UpdateError::Relaunch(std::io::Error::new(
        std::io::ErrorKind::PermissionDenied,
        "denied",
    ))
    .to_string();
    assert_eq!(message, "relaunch failed: denied");
}
