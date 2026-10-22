//! `FilesConfig` as an injected value: built by `new` without a profile, with
//! the storage tree created by an explicit `ensure_storage_structure` step.

use systemprompt_files::{FilesConfig, FilesConfigYaml};

#[test]
fn new_derives_paths_and_urls_from_its_parts() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let yaml: FilesConfigYaml =
        serde_yaml::from_str("urlPrefix: /cdn\ncacheControl: public, max-age=60")
            .expect("files yaml parses");
    let cfg = FilesConfig::new(tmp.path().to_path_buf(), yaml);

    assert_eq!(cfg.storage(), tmp.path());
    assert_eq!(cfg.url_prefix(), "/cdn");
    assert_eq!(cfg.cache_control(), Some("public, max-age=60"));
    assert_eq!(cfg.files(), tmp.path().join("files"));
    assert_eq!(cfg.file_url("/a/b.txt"), "/cdn/files/a/b.txt");
    assert_eq!(
        cfg.generated_image_url("x.png"),
        "/cdn/images/generated/x.png"
    );
}

#[test]
fn validate_rejects_a_relative_storage_root_and_keeps_the_filesystem_untouched() {
    let cfg = FilesConfig::new(
        std::path::PathBuf::from("relative/storage"),
        FilesConfigYaml::default(),
    );

    assert!(
        cfg.validate().is_err(),
        "a relative storage root is invalid"
    );
    assert!(
        !std::path::Path::new("relative/storage").exists(),
        "validate never creates directories"
    );
}

#[test]
fn ensure_storage_structure_is_an_explicit_step_after_validate() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let root = tmp.path().join("storage");
    let cfg = FilesConfig::new(root.clone(), FilesConfigYaml::default());

    cfg.validate().expect("absolute root validates");
    assert!(!root.exists(), "validate leaves the tree alone");

    cfg.ensure_storage_structure().expect("tree is created");
    assert!(cfg.files().is_dir());
    assert!(cfg.images().is_dir());
}
