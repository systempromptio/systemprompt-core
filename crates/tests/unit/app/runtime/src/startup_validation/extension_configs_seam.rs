//! Direct tests of the public `validate_extension_configs` seam, which runs
//! the same per-extension load-and-validate pass as the serve boot path but
//! needs neither a profile nor a `Config`. The inventory fixture extensions
//! (`ext_fixtures`) provide one always-valid and one always-rejecting
//! config-bearing extension.

use systemprompt_runtime::validate_extension_configs;

#[test]
fn missing_config_files_fall_back_to_empty_config() {
    let tmp = tempfile::tempdir().expect("tempdir");

    let outcomes = validate_extension_configs(tmp.path()).expect("registry discovery succeeds");

    let ok = outcomes
        .iter()
        .find(|o| o.extension_id == "covextok")
        .expect("covextok outcome present");
    assert_eq!(ok.config_key, "covextok.config");
    assert_eq!(
        ok.error, None,
        "an absent config file validates as an empty config"
    );

    let bad = outcomes
        .iter()
        .find(|o| o.extension_id == "covextbad")
        .expect("covextbad outcome present");
    assert_eq!(bad.config_key, "covextbad.config");
    assert!(
        bad.error
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

    let outcomes = validate_extension_configs(tmp.path()).expect("registry discovery succeeds");

    let ok = outcomes
        .iter()
        .find(|o| o.extension_id == "covextok")
        .expect("covextok outcome present");
    assert!(
        ok.error
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

    let outcomes = validate_extension_configs(tmp.path()).expect("registry discovery succeeds");

    let ok = outcomes
        .iter()
        .find(|o| o.extension_id == "covextok")
        .expect("covextok outcome present");
    assert_eq!(ok.error, None);
}
