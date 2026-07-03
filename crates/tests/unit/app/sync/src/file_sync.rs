//! Tests for `FileSyncService::apply` / `backup_services` and the
//! `SyncDiffResult` / `FileDiffStatus` helpers. None of these paths require
//! a live cloud API, only a temporary directory on disk.

use flate2::Compression;
use flate2::write::GzEncoder;
use std::fs;
use std::io::Write;
use systemprompt_sync::{FileDiffStatus, FileSyncService, SyncDiffEntry, SyncDiffResult};
use tar::Builder;
use tempfile::TempDir;

mod file_diff_status {
    use super::*;

    #[test]
    fn equality_and_copy() {
        let a = FileDiffStatus::Added;
        let b = a;
        assert_eq!(a, b);
        assert_ne!(FileDiffStatus::Added, FileDiffStatus::Deleted);
        assert_ne!(FileDiffStatus::Modified, FileDiffStatus::Unchanged);
    }

    #[test]
    fn debug_renders_variant() {
        assert!(format!("{:?}", FileDiffStatus::Modified).contains("Modified"));
    }

    #[test]
    fn serialise_roundtrip() {
        let json = serde_json::to_string(&FileDiffStatus::Unchanged).expect("ser");
        let back: FileDiffStatus = serde_json::from_str(&json).expect("de");
        assert_eq!(back, FileDiffStatus::Unchanged);
    }
}

mod sync_diff_result {
    use super::*;

    fn make(entries: Vec<SyncDiffEntry>) -> SyncDiffResult {
        let mut added = 0;
        let mut modified = 0;
        let mut deleted = 0;
        let mut unchanged = 0;
        for e in &entries {
            match e.status {
                FileDiffStatus::Added => added += 1,
                FileDiffStatus::Modified => modified += 1,
                FileDiffStatus::Deleted => deleted += 1,
                FileDiffStatus::Unchanged => unchanged += 1,
            }
        }
        SyncDiffResult {
            entries,
            added,
            modified,
            deleted,
            unchanged,
        }
    }

    #[test]
    fn empty_has_no_changes() {
        let r = make(vec![]);
        assert!(!r.has_changes());
        assert!(r.changed_paths().is_empty());
    }

    #[test]
    fn unchanged_only_has_no_changes() {
        let r = make(vec![SyncDiffEntry {
            path: "a.txt".to_owned(),
            status: FileDiffStatus::Unchanged,
            size: 1,
        }]);
        assert!(!r.has_changes());
        assert!(r.changed_paths().is_empty());
    }

    #[test]
    fn any_added_modified_or_deleted_has_changes() {
        for st in [
            FileDiffStatus::Added,
            FileDiffStatus::Modified,
            FileDiffStatus::Deleted,
        ] {
            let r = make(vec![SyncDiffEntry {
                path: format!("{st:?}.txt"),
                status: st,
                size: 1,
            }]);
            assert!(r.has_changes(), "expected changes for {st:?}");
        }
    }

    #[test]
    fn changed_paths_excludes_unchanged() {
        let r = make(vec![
            SyncDiffEntry {
                path: "kept.txt".to_owned(),
                status: FileDiffStatus::Unchanged,
                size: 1,
            },
            SyncDiffEntry {
                path: "new.txt".to_owned(),
                status: FileDiffStatus::Added,
                size: 2,
            },
            SyncDiffEntry {
                path: "gone.txt".to_owned(),
                status: FileDiffStatus::Deleted,
                size: 0,
            },
        ]);
        let paths = r.changed_paths();
        assert_eq!(paths.len(), 2);
        assert!(paths.iter().any(|p| p == "new.txt"));
        assert!(paths.iter().any(|p| p == "gone.txt"));
        assert!(!paths.iter().any(|p| p == "kept.txt"));
    }
}

mod backup_services {
    use super::*;

    #[test]
    fn writes_zip_with_included_subdirs() {
        let tmp = TempDir::new().expect("tmp");
        let project_root = tmp.path();
        let services = project_root.join("services");
        fs::create_dir_all(services.join("agents")).expect("mkdir agents");
        fs::write(services.join("agents/a.yaml"), "name: a\n").expect("write");
        // A non-included dir is silently skipped.
        fs::create_dir_all(services.join("excluded_dir")).expect("mkdir excluded");
        fs::write(services.join("excluded_dir/x.txt"), "ignored").expect("write");

        let zip_path = FileSyncService::backup_services(&services).expect("backup");
        assert!(zip_path.exists(), "zip should be created");
        assert!(zip_path.to_string_lossy().ends_with(".zip"));
        // The zip lives in <project_root>/backup/<timestamp>.zip
        assert_eq!(zip_path.parent().unwrap().file_name().unwrap(), "backup");

        // The zip must contain the agents file but not the excluded dir.
        let bytes = fs::read(&zip_path).expect("read zip");
        let mut reader = std::io::Cursor::new(&bytes);
        let mut zip = zip::ZipArchive::new(&mut reader).expect("open zip");
        let names: Vec<String> = (0..zip.len())
            .map(|i| zip.by_index(i).expect("entry").name().to_owned())
            .collect();
        assert!(
            names.iter().any(|n| n.contains("agents/a.yaml")),
            "{names:?}"
        );
        assert!(
            !names.iter().any(|n| n.contains("excluded_dir")),
            "{names:?}"
        );
    }
}

mod apply_tarball {
    use super::*;

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

    #[test]
    fn apply_extracts_all_entries() {
        let tarball = build_tarball(&[("agents/a.yaml", b"agent: 1"), ("skills/s.md", b"# skill")]);
        let tmp = TempDir::new().expect("tmp");
        let extracted = FileSyncService::apply(&tarball, tmp.path(), None).expect("apply");
        assert!(extracted >= 1);
        assert!(tmp.path().join("agents/a.yaml").exists());
        assert!(tmp.path().join("skills/s.md").exists());
    }

    #[test]
    fn apply_selective_only_extracts_requested_paths() {
        let tarball = build_tarball(&[
            ("agents/a.yaml", b"agent: 1"),
            ("skills/s.md", b"# skill"),
            ("content/c.md", b"body"),
        ]);
        let tmp = TempDir::new().expect("tmp");
        let only = ["agents/a.yaml".to_owned()];
        let extracted = FileSyncService::apply(&tarball, tmp.path(), Some(&only)).expect("apply");
        assert_eq!(
            extracted, 1,
            "only the single filtered path should be extracted"
        );
        assert!(tmp.path().join("agents/a.yaml").exists());
        assert_eq!(
            std::fs::read(tmp.path().join("agents/a.yaml")).expect("read extracted file"),
            b"agent: 1"
        );
        assert!(!tmp.path().join("skills/s.md").exists());
        assert!(!tmp.path().join("content/c.md").exists());
    }

    #[test]
    fn apply_empty_tarball_succeeds_with_zero_entries() {
        let tarball = build_tarball(&[]);
        let tmp = TempDir::new().expect("tmp");
        let extracted = FileSyncService::apply(&tarball, tmp.path(), None).expect("apply");
        assert_eq!(extracted, 0);
    }
}

#[test]
fn drop_writer_to_silence_unused() {
    // GzEncoder needs flush via Write trait import.
    let mut w: Vec<u8> = Vec::new();
    w.write_all(b"").unwrap();
}
