use std::path::Path;

use systemprompt_bridge::gateway::types::ReleaseManifest;
use systemprompt_bridge::instance::window::CombinedWindow;
use systemprompt_bridge::update::staged::{
    hash_program, pending, read, reconcile, record, record_installed,
};
use systemprompt_bridge::update::{StagedUpdate, UpdateError, UpdateStatus, decide};
use tempfile::TempDir;

fn manifest(version: &str) -> ReleaseManifest {
    ReleaseManifest {
        version: version.to_owned(),
        sha256: "0".repeat(64),
        size: 1,
        notes_url: None,
    }
}

fn staged(dir: &Path, version: &str) -> StagedUpdate {
    let program = dir.join("program");
    std::fs::write(&program, format!("build {version}")).expect("seed program");
    StagedUpdate {
        version: version.to_owned(),
        sha256: hash_program(&program).expect("hash program"),
        program,
    }
}

fn decide_staged(local: &str, staged: &str, offered: &str) -> UpdateStatus {
    decide(
        local,
        Some(staged),
        &manifest(offered),
        &CombinedWindow::Open,
    )
    .expect("decides")
}

#[test]
fn an_offer_equal_to_the_staged_release_is_staged() {
    assert_eq!(
        decide_staged("0.77.0", "0.79.0", "0.79.0"),
        UpdateStatus::Staged {
            version: "0.79.0".to_owned()
        }
    );
}

fn bounded(min: Option<&str>, max: Option<&str>) -> CombinedWindow {
    use systemprompt_models::bridge::instance::BridgeVersionWindow;
    CombinedWindow::Window(BridgeVersionWindow {
        min: min.map(|v| semver::Version::parse(v).expect("min")),
        max: max.map(|v| semver::Version::parse(v).expect("max")),
        enforce_max: true,
    })
}

#[test]
fn an_offer_older_than_the_staged_release_but_newer_than_the_running_one_is_installed() {
    assert!(matches!(
        decide_staged("0.77.0", "0.79.0", "0.78.0"),
        UpdateStatus::Available { ref version, .. } if version == "0.78.0"
    ));
}

#[test]
fn an_offer_of_the_running_release_replaces_a_withdrawn_staged_one() {
    assert!(matches!(
        decide_staged("0.77.0", "0.79.0", "0.77.0"),
        UpdateStatus::Available { ref version, .. } if version == "0.77.0"
    ));
}

#[test]
fn an_enforced_downgrade_below_the_running_release_is_installed_over_the_staged_one() {
    let status = decide(
        "0.77.0",
        Some("0.79.0"),
        &manifest("0.76.0"),
        &bounded(None, Some("0.76.0")),
    )
    .expect("decides");
    assert!(
        matches!(status, UpdateStatus::Available { ref version, .. } if version == "0.76.0"),
        "{status:?}"
    );
}

#[test]
fn an_older_offer_without_enforcement_keeps_the_running_release_current() {
    assert!(matches!(
        decide_staged("0.77.0", "0.79.0", "0.76.0"),
        UpdateStatus::Current { ref version } if version == "0.77.0"
    ));
}

#[test]
fn a_staged_release_outside_the_instance_window_is_not_reported_ready() {
    let err = decide(
        "0.77.0",
        Some("0.79.0"),
        &manifest("0.79.0"),
        &bounded(None, Some("0.78.0")),
    )
    .expect_err("outside the window");
    assert!(matches!(err, UpdateError::OutsideWindow(_)), "{err:?}");
}

#[test]
fn an_offer_newer_than_the_staged_release_is_available() {
    assert!(matches!(
        decide_staged("0.77.0", "0.79.0", "0.80.0"),
        UpdateStatus::Available { ref version, .. } if version == "0.80.0"
    ));
}

#[test]
fn a_staged_release_no_newer_than_the_running_one_is_ignored() {
    assert!(matches!(
        decide_staged("0.79.0", "0.79.0", "0.79.0"),
        UpdateStatus::Current { .. }
    ));
    assert!(matches!(
        decide_staged("0.80.0", "0.79.0", "0.81.0"),
        UpdateStatus::Available { ref version, .. } if version == "0.81.0"
    ));
}

#[test]
fn a_recorded_release_reads_back_unchanged() {
    let dir = TempDir::new().expect("dir");
    let path = dir.path().join("staged-update.json");
    record(&path, &staged(dir.path(), "0.79.0")).expect("record");
    assert_eq!(read(&path), Some(staged(dir.path(), "0.79.0")));
    assert_eq!(pending(&path, "0.77.0"), Some(staged(dir.path(), "0.79.0")));
}

#[test]
fn a_malformed_record_reads_as_nothing_staged() {
    let dir = TempDir::new().expect("dir");
    let path = dir.path().join("staged-update.json");
    std::fs::write(&path, b"{not json").expect("seed");
    assert_eq!(read(&path), None);
    assert_eq!(pending(&path, "0.77.0"), None);
    assert_eq!(reconcile(&path, "0.77.0").expect("reconciles"), None);
    assert!(!path.exists(), "a malformed record is cleared at startup");
}

#[test]
fn a_missing_record_reads_as_nothing_staged() {
    let dir = TempDir::new().expect("dir");
    let path = dir.path().join("staged-update.json");
    assert_eq!(read(&path), None);
    assert_eq!(reconcile(&path, "0.77.0").expect("reconciles"), None);
}

#[test]
fn startup_clears_the_record_once_the_staged_release_is_running() {
    let dir = TempDir::new().expect("dir");
    let path = dir.path().join("staged-update.json");
    record(&path, &staged(dir.path(), "0.79.0")).expect("record");
    assert_eq!(reconcile(&path, "0.79.0").expect("reconciles"), None);
    assert!(
        !path.exists(),
        "running the staged release clears its record"
    );

    record(&path, &staged(dir.path(), "0.79.0")).expect("record");
    assert_eq!(reconcile(&path, "0.80.0").expect("reconciles"), None);
    assert!(!path.exists(), "running a newer release clears the record");
}

#[test]
fn startup_keeps_the_record_while_the_staged_release_waits() {
    let dir = TempDir::new().expect("dir");
    let path = dir.path().join("staged-update.json");
    record(&path, &staged(dir.path(), "0.79.0")).expect("record");
    assert_eq!(
        reconcile(&path, "0.77.0").expect("reconciles"),
        Some(staged(dir.path(), "0.79.0"))
    );
    assert!(path.exists());
}

#[test]
fn a_record_whose_program_was_replaced_is_not_honoured() {
    let dir = TempDir::new().expect("dir");
    let path = dir.path().join("staged-update.json");
    let recorded = staged(dir.path(), "0.79.0");
    record(&path, &recorded).expect("record");
    std::fs::write(&recorded.program, b"reverted by hand").expect("replace program");
    assert_eq!(pending(&path, "0.77.0"), None);
    assert_eq!(reconcile(&path, "0.77.0").expect("reconciles"), None);
    assert!(
        !path.exists(),
        "a record for a program no longer installed is cleared"
    );
}

#[test]
fn a_record_whose_program_is_missing_is_not_honoured() {
    let dir = TempDir::new().expect("dir");
    let path = dir.path().join("staged-update.json");
    let recorded = staged(dir.path(), "0.79.0");
    record(&path, &recorded).expect("record");
    std::fs::remove_file(&recorded.program).expect("remove program");
    assert_eq!(pending(&path, "0.77.0"), None);
}

#[test]
fn recording_an_install_hashes_the_installed_program() {
    let dir = TempDir::new().expect("dir");
    let path = dir.path().join("staged-update.json");
    let program = dir.path().join("bridge");
    std::fs::write(&program, b"new build").expect("seed program");
    let recorded = record_installed(&path, "0.79.0", &program).expect("record install");
    assert_eq!(recorded.program, program);
    assert_eq!(
        recorded.sha256,
        hash_program(&program).expect("hash program")
    );
    assert_eq!(pending(&path, "0.77.0"), Some(recorded));
}
