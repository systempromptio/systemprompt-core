//! Exact-value assertions for every `FilesConfig` path and URL accessor.

use systemprompt_files::FilesConfig;
use systemprompt_test_fixtures::ensure_test_bootstrap;

#[test]
fn path_and_url_accessors_derive_from_storage_and_prefix() {
    let b = ensure_test_bootstrap();
    let cfg = FilesConfig::from_profile(b.boot.profile(), &b.app_paths).expect("from_profile");
    let root = &b.storage_path;

    assert_eq!(cfg.generated_images(), root.join("files/images/generated"));
    assert_eq!(cfg.content_images("blog"), root.join("files/images/blog"));
    assert_eq!(cfg.images(), root.join("files/images"));
    assert_eq!(cfg.files(), root.join("files"));
    assert_eq!(cfg.audio(), root.join("files/audio"));
    assert_eq!(cfg.video(), root.join("files/video"));
    assert_eq!(cfg.documents(), root.join("files/documents"));
    assert_eq!(cfg.uploads(), root.join("files/uploads"));

    assert_eq!(cfg.url_prefix(), "/files");
    assert_eq!(cfg.public_url("/a/b.png"), "/files/a/b.png");
    assert_eq!(cfg.image_url("/x.png"), "/files/images/x.png");
    assert_eq!(
        cfg.generated_image_url("/g.png"),
        "/files/images/generated/g.png"
    );
    assert_eq!(
        cfg.content_image_url("blog", "/c.png"),
        "/files/images/blog/c.png"
    );
    assert_eq!(cfg.file_url("/f.bin"), "/files/files/f.bin");
    assert_eq!(cfg.audio_url("/a.mp3"), "/files/files/audio/a.mp3");
    assert_eq!(cfg.video_url("/v.mp4"), "/files/files/video/v.mp4");
    assert_eq!(cfg.document_url("/d.pdf"), "/files/files/documents/d.pdf");
    assert_eq!(cfg.upload_url("/u.png"), "/files/files/uploads/u.png");
}
