//! Exercises remote-vs-local diffing and rejection paths in
//! `extract_tarball` / `compare_tarball_with_local` reached via
//! `FileSyncService::apply` and `FileSyncService::download_and_diff` is
//! covered by the integration suite; here we hit the bundler directly
//! through `apply` + a curated tarball.

use flate2::Compression;
use flate2::write::GzEncoder;
use std::io::Write;
use systemprompt_sync::FileSyncService;
use tar::Builder;
use tempfile::TempDir;

fn build_tarball(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    {
        let mut tar = Builder::new(&mut encoder);
        for (name, content) in entries {
            let mut header = tar::Header::new_gnu();
            header.set_size(content.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            tar.append_data(&mut header, name, *content)
                .expect("append");
        }
        tar.finish().expect("finish");
    }
    encoder.finish().expect("encode")
}

fn build_tarball_dir(entries: &[(&str, &[u8])], dirs: &[&str]) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    {
        let mut tar = Builder::new(&mut encoder);
        for dir in dirs {
            let mut header = tar::Header::new_gnu();
            header.set_size(0);
            header.set_mode(0o755);
            header.set_entry_type(tar::EntryType::Directory);
            header.set_cksum();
            tar.append_data(&mut header, *dir, std::io::empty())
                .expect("append dir");
        }
        for (name, content) in entries {
            let mut header = tar::Header::new_gnu();
            header.set_size(content.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            tar.append_data(&mut header, name, *content)
                .expect("append");
        }
        tar.finish().expect("finish");
    }
    encoder.finish().expect("encode")
}

#[test]
fn apply_rejects_disallowed_first_segment_with_subpath() {
    // `tar::Builder::append_data` rejects absolute paths and `..`
    // components at the Rust level, so we exercise the extractor's
    // INCLUDE_DIRS guard via a path that *is* relative and `..`-free
    // but whose first segment isn't allow-listed.
    let tarball = build_tarball(&[("etc/passwd", b"bad")]);
    let tmp = TempDir::new().expect("tmp");
    let err = FileSyncService::apply(&tarball, tmp.path(), None)
        .expect_err("non-allowed top-level dir must be rejected");
    let msg = err.to_string().to_lowercase();
    assert!(
        msg.contains("allowed") || msg.contains("top-level") || msg.contains("unsafe"),
        "unexpected error: {msg}"
    );
}

#[test]
fn apply_rejects_disallowed_top_level_dir() {
    let tarball = build_tarball(&[("forbidden_dir/file.yaml", b"data")]);
    let tmp = TempDir::new().expect("tmp");
    let err = FileSyncService::apply(&tarball, tmp.path(), None)
        .expect_err("non-allowed top-level dir must be rejected");
    let msg = err.to_string().to_lowercase();
    assert!(
        msg.contains("allowed") || msg.contains("top-level") || msg.contains("unsafe"),
        "unexpected error: {msg}"
    );
}

#[test]
fn apply_accepts_directory_entries_in_allowed_paths() {
    let tarball = build_tarball_dir(&[("agents/a.yaml", b"agent: 1")], &["agents/"]);
    let tmp = TempDir::new().expect("tmp");
    let _ = FileSyncService::apply(&tarball, tmp.path(), None).expect("apply");
    assert!(tmp.path().join("agents/a.yaml").exists());
}

#[test]
fn apply_selective_skips_paths_outside_filter() {
    let tarball = build_tarball(&[("agents/a.yaml", b"agent"), ("agents/b.yaml", b"agent2")]);
    let tmp = TempDir::new().expect("tmp");
    let only = ["agents/a.yaml".to_owned()];
    let extracted = FileSyncService::apply(&tarball, tmp.path(), Some(&only)).expect("apply");
    assert!(tmp.path().join("agents/a.yaml").exists());
    assert!(!tmp.path().join("agents/b.yaml").exists());
    assert_eq!(extracted, 1);
}

#[test]
fn apply_creates_nested_parent_directories() {
    let tarball = build_tarball(&[("agents/sub/deep/file.yaml", b"x")]);
    let tmp = TempDir::new().expect("tmp");
    let extracted = FileSyncService::apply(&tarball, tmp.path(), None).expect("apply");
    assert_eq!(extracted, 1);
    assert!(tmp.path().join("agents/sub/deep/file.yaml").exists());
}

#[test]
fn drop_writer_to_silence_unused() {
    let mut w: Vec<u8> = Vec::new();
    w.write_all(b"").unwrap();
}
