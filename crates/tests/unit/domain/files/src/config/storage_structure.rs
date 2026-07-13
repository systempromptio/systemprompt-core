//! `FilesConfig::from_profile` derivation and `ensure_storage_structure`
//! success/error branches, driven off the tempdir-backed bootstrap profile.

use systemprompt_files::{FilePersistenceMode, FilesConfig};
use systemprompt_test_fixtures::{TestBootstrap, ensure_test_bootstrap};

fn config_with_yaml(content: Option<&str>) -> (&'static TestBootstrap, FilesConfig) {
    let b = ensure_test_bootstrap();
    if let Some(yaml) = content {
        std::fs::write(b.services_path.join("config/files.yaml"), yaml).expect("write files.yaml");
    }
    let cfg = FilesConfig::from_profile(&b.app_paths).expect("from_profile");
    (b, cfg)
}

#[test]
fn from_profile_reads_yaml_values() {
    let (b, cfg) = config_with_yaml(Some(
        "files:\n  urlPrefix: /assets\n  upload:\n    enabled: false\n    max_file_size_bytes: 1234\n    persistence_mode: user_library\n",
    ));

    assert_eq!(cfg.url_prefix(), "/assets");
    assert!(!cfg.upload().enabled);
    assert_eq!(cfg.upload().max_file_size_bytes, 1234);
    assert_eq!(
        cfg.upload().persistence_mode,
        FilePersistenceMode::UserLibrary
    );
    assert_eq!(cfg.storage(), b.storage_path.as_path());
    cfg.validate().expect("absolute storage root validates");
}

#[test]
fn ensure_storage_structure_creates_missing_dirs() {
    let (b, cfg) = config_with_yaml(None);
    std::fs::remove_dir_all(&b.storage_path).expect("remove storage root");

    let errors = cfg.ensure_storage_structure();

    assert!(errors.is_empty(), "unexpected errors: {errors:?}");
    assert!(cfg.files().is_dir());
    assert!(cfg.images().is_dir());
}

#[test]
fn ensure_storage_structure_reports_uncreatable_subdirs() {
    let (_b, cfg) = config_with_yaml(None);
    // A regular file at each subdir path makes create_dir_all fail.
    std::fs::write(cfg.files(), b"blocker").expect("blocker file at files/");

    let errors = cfg.ensure_storage_structure();

    // files/ "exists" (as the blocker file) so only files/images/ fails.
    assert_eq!(errors.len(), 1, "unexpected errors: {errors:?}");
    assert!(errors[0].contains("Failed to create"));
    assert!(errors[0].contains(&cfg.images().display().to_string()));
}
