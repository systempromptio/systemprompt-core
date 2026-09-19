//! Windows sharing guards, without machine-owner or elevation requirements.

use std::os::windows::fs::OpenOptionsExt;

use systemprompt_bridge::fsutil::open_read_guard;

#[test]
fn read_guard_blocks_directory_and_file_renames_until_dropped() {
    let root = tempfile::tempdir().unwrap();
    for directory in [false, true] {
        let path = root
            .path()
            .join(if directory { "directory" } else { "file" });
        let moved = path.with_extension("moved");
        if directory {
            std::fs::create_dir(&path).unwrap();
        } else {
            std::fs::write(&path, b"contents").unwrap();
        }
        let guard = open_read_guard(&path, directory).unwrap();
        assert!(std::fs::rename(&path, &moved).is_err());
        assert!(
            if directory {
                std::fs::remove_dir(&path)
            } else {
                std::fs::remove_file(&path)
            }
            .is_err()
        );
        drop(guard);
        std::fs::rename(&path, &moved).unwrap();
    }
}

#[test]
fn read_guard_refuses_an_existing_delete_handle() {
    let root = tempfile::tempdir().unwrap();
    for directory in [false, true] {
        let path = root
            .path()
            .join(if directory { "directory" } else { "file" });
        if directory {
            std::fs::create_dir(&path).unwrap();
        } else {
            std::fs::write(&path, b"contents").unwrap();
        }
        // DELETE and FILE_FLAG_BACKUP_SEMANTICS from the Windows file API.
        let deletion = std::fs::OpenOptions::new()
            .access_mode(0x0001_0000)
            .custom_flags(0x0200_0000)
            .open(&path)
            .unwrap();
        assert!(open_read_guard(&path, directory).is_err());
        drop(deletion);
        assert!(open_read_guard(&path, directory).is_ok());
    }
}
