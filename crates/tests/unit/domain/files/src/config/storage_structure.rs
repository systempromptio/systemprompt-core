//! `FilesConfig::from_profile` derivation, the `FilesConfig::new` form the
//! builder and fixtures use, and `ensure_storage_structure` success/error
//! branches, driven off the tempdir-backed bootstrap profile.

use systemprompt_files::{FilePersistenceMode, FilesConfig, FilesConfigYaml, FilesError};
use systemprompt_test_fixtures::{TestBootstrap, ensure_test_bootstrap};

fn config_with_yaml(content: Option<&str>) -> (&'static TestBootstrap, FilesConfig) {
    let b = ensure_test_bootstrap();
    if let Some(yaml) = content {
        std::fs::write(b.services_path.join("config/files.yaml"), yaml).expect("write files.yaml");
    }
    let cfg = FilesConfig::from_profile(b.boot.profile(), &b.app_paths).expect("from_profile");
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

    cfg.ensure_storage_structure()
        .expect("missing storage dirs are created");
    assert!(cfg.files().is_dir());
    assert!(cfg.images().is_dir());
}

#[test]
fn ensure_storage_structure_reports_uncreatable_subdirs() {
    let (_b, cfg) = config_with_yaml(None);
    // A regular file at each subdir path makes create_dir_all fail.
    std::fs::write(cfg.files(), b"blocker").expect("blocker file at files/");

    let err = cfg
        .ensure_storage_structure()
        .expect_err("a file where a directory must go is reported");

    // files/ "exists" (as the blocker file) so files/images/ is the one that fails.
    match err {
        FilesError::StorageDir { path, .. } => assert_eq!(path, cfg.images()),
        other => panic!("expected StorageDir, got {other:?}"),
    }
    std::fs::remove_file(cfg.files()).expect("remove blocker");
}

#[test]
fn new_builds_the_same_config_without_a_profile() {
    let (b, from_profile) = config_with_yaml(None);
    let direct = FilesConfig::new(b.storage_path.clone(), FilesConfigYaml::default());

    assert_eq!(direct.storage(), from_profile.storage());
    assert_eq!(direct.url_prefix(), "/files");
    assert_eq!(direct.files(), from_profile.files());
    assert_eq!(direct.upload().enabled, from_profile.upload().enabled);
}
