//! Integration tests for FilesConfig getters and URL formatters.
//!
//! Bootstraps `ProfileBootstrap` + `FilesConfig` via the shared `bootstrap`
//! helper so the URL helpers can be exercised against a real storage root.


use crate::bootstrap::test_env;

#[test]
fn files_config_storage_tree_paths_resolve() {
    let cfg = &test_env().files_config;

    let storage = cfg.storage();
    assert!(storage.is_absolute(), "storage root is absolute");

    for child in [
        cfg.files(),
        cfg.images(),
        cfg.generated_images(),
        cfg.audio(),
        cfg.video(),
        cfg.documents(),
        cfg.uploads(),
        cfg.content_images("blog"),
    ] {
        assert!(
            child.starts_with(storage),
            "child path {:?} must be under storage root {:?}",
            child,
            storage
        );
    }
}

#[test]
fn files_config_url_formatters_strip_leading_slash() {
    let cfg = &test_env().files_config;

    let prefix = cfg.url_prefix().to_owned();

    assert_eq!(
        cfg.public_url("/foo/bar.png"),
        format!("{prefix}/foo/bar.png"),
    );
    assert_eq!(cfg.public_url("foo.png"), format!("{prefix}/foo.png"));
    assert_eq!(
        cfg.image_url("/banner.png"),
        format!("{prefix}/images/banner.png"),
    );
    assert_eq!(
        cfg.generated_image_url("ai-001.png"),
        format!("{prefix}/images/generated/ai-001.png"),
    );
    assert_eq!(
        cfg.content_image_url("blog", "post.jpg"),
        format!("{prefix}/images/blog/post.jpg"),
    );
    assert_eq!(
        cfg.file_url("docs/spec.pdf"),
        format!("{prefix}/files/docs/spec.pdf"),
    );
    assert_eq!(
        cfg.audio_url("track.mp3"),
        format!("{prefix}/files/audio/track.mp3"),
    );
    assert_eq!(
        cfg.video_url("clip.mp4"),
        format!("{prefix}/files/video/clip.mp4"),
    );
    assert_eq!(
        cfg.document_url("notes.txt"),
        format!("{prefix}/files/documents/notes.txt"),
    );
    assert_eq!(
        cfg.upload_url("avatar.png"),
        format!("{prefix}/files/uploads/avatar.png"),
    );
}

#[test]
fn files_config_validate_succeeds_for_initialised_config() {
    let cfg = &test_env().files_config;
    cfg.validate().expect("absolute storage root validates");
}

#[test]
fn files_config_ensure_storage_structure_creates_subdirs() {
    let cfg = &test_env().files_config;

    cfg.ensure_storage_structure()
        .expect("ensure_storage_structure creates the tree");
    assert!(cfg.files().exists(), "files dir created");
    assert!(cfg.images().exists(), "images dir created");
}

#[test]
fn files_config_upload_returns_config() {
    let cfg = &test_env().files_config;
    let upload = cfg.upload();
    assert!(upload.enabled);
    assert!(upload.max_file_size_bytes > 0);
}

#[test]
fn files_config_validator_load_and_validate() {
    use systemprompt_files::FilesConfigValidator;
    use systemprompt_manifest::validators::DomainConfig;

    let _env = test_env();

    let v = FilesConfigValidator::new();
    let err = v
        .validate()
        .expect_err("validator with no config should error");
    let _ = format!("{err:?}");
    assert_eq!(v.domain_id(), "files");
    assert!(v.priority() > 0);
}
