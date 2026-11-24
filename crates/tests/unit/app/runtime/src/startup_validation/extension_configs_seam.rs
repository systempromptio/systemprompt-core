//! Direct tests of the public `AdmittedConfigs::validate` seam, which runs
//! the same per-extension load-and-validate pass as the serve boot path but
//! needs neither a profile nor a `Config`. The fixture members
//! (`ext_fixtures::fixture_admitted`) provide one always-valid and one
//! always-rejecting config section.

use crate::ext_fixtures::fixture_admitted;

#[test]
fn missing_config_files_fall_back_to_empty_config() {
    let tmp = tempfile::tempdir().expect("tempdir");

    let outcomes = systemprompt_extension::AdmittedConfigs::of(fixture_admitted().built())
        .validate(tmp.path());

    let ok = outcomes
        .iter()
        .find(|o| o.extension_id == "covextok")
        .expect("covextok outcome present");
    assert_eq!(ok.config_key, "covextok.config");
    assert!(
        ok.error.is_none(),
        "an absent config file validates as an empty config"
    );

    let bad = outcomes
        .iter()
        .find(|o| o.extension_id == "covextbad")
        .expect("covextbad outcome present");
    assert_eq!(bad.config_key, "covextbad.config");
    assert!(
        bad.error
            .as_ref()
            .map(ToString::to_string)
            .as_deref()
            .is_some_and(|e| e.contains("fixture always rejects")),
        "got: {:?}",
        bad.error
    );

    assert!(
        !outcomes.iter().any(|o| o.extension_id == "covassets_ok"),
        "asset-only extensions carry no config outcome"
    );
}

#[test]
fn malformed_config_file_reports_load_error() {
    let tmp = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(tmp.path().join("config")).expect("mkdir config");
    std::fs::write(tmp.path().join("config/covextok.yaml"), ": : not yaml [")
        .expect("write malformed config");

    let outcomes = systemprompt_extension::AdmittedConfigs::of(fixture_admitted().built())
        .validate(tmp.path());

    let ok = outcomes
        .iter()
        .find(|o| o.extension_id == "covextok")
        .expect("covextok outcome present");
    assert!(
        ok.error
            .as_ref()
            .map(ToString::to_string)
            .as_deref()
            .is_some_and(|e| e.contains("Cannot parse") && e.contains("covextok.yaml")),
        "got: {:?}",
        ok.error
    );
}

#[test]
fn valid_config_file_passes_validation() {
    let tmp = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(tmp.path().join("config")).expect("mkdir config");
    std::fs::write(tmp.path().join("config/covextok.yaml"), "mode: fine\n")
        .expect("write valid config");

    let outcomes = systemprompt_extension::AdmittedConfigs::of(fixture_admitted().built())
        .validate(tmp.path());

    let ok = outcomes
        .iter()
        .find(|o| o.extension_id == "covextok")
        .expect("covextok outcome present");
    assert!(ok.error.is_none());
}

#[test]
fn a_rejected_extension_config_file_is_reported_by_the_startup_pass() {
    let tmp = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(tmp.path().join("config")).expect("mkdir config");
    std::fs::write(tmp.path().join("config/covextbad.yaml"), "mode: fine\n").expect("write config");

    let outcomes = systemprompt_extension::AdmittedConfigs::of(fixture_admitted().built())
        .validate(tmp.path());

    let bad = outcomes
        .iter()
        .find(|o| o.extension_id == "covextbad")
        .expect("covextbad outcome present");
    assert!(
        bad.error
            .as_ref()
            .map(ToString::to_string)
            .as_deref()
            .is_some_and(|e| e.contains("fixture always rejects")),
        "the member's own config document must reach its config validation, got: {:?}",
        bad.error
    );
}
