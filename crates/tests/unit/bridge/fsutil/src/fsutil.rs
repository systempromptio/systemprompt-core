use std::fs;

use systemprompt_bridge::fsutil::{
    atomic_write_0600, copy_dir_recursive, create_dir_all_mode_0700, read_optional, temp_path_for,
};
use tempfile::tempdir;

#[cfg(unix)]
fn mode_of(path: &std::path::Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path).unwrap().permissions().mode() & 0o777
}

#[test]
fn atomic_write_creates_new_file_with_exact_bytes() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("new.txt");
    let bytes = b"hello world\x00\x01\x02";

    atomic_write_0600(&path, bytes).unwrap();

    assert!(path.exists());
    assert_eq!(fs::read(&path).unwrap(), bytes);
}

#[test]
fn atomic_write_overwrites_existing_file() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("existing.txt");

    fs::write(&path, b"old contents that are longer").unwrap();
    atomic_write_0600(&path, b"new").unwrap();

    assert_eq!(fs::read(&path).unwrap(), b"new");
}

#[test]
fn atomic_write_creates_missing_parent_dirs() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("a").join("b").join("c").join("file.txt");

    assert!(!path.parent().unwrap().exists());
    atomic_write_0600(&path, b"deep").unwrap();

    assert!(path.exists());
    assert_eq!(fs::read(&path).unwrap(), b"deep");
}

#[cfg(unix)]
#[test]
fn atomic_write_sets_mode_0600() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("perm.txt");

    atomic_write_0600(&path, b"secret").unwrap();

    assert_eq!(mode_of(&path), 0o600);
}

#[cfg(unix)]
#[test]
fn atomic_write_overwrite_tightens_mode_to_0600() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempdir().unwrap();
    let path = dir.path().join("loose.txt");

    fs::write(&path, b"old").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();

    atomic_write_0600(&path, b"new").unwrap();

    assert_eq!(mode_of(&path), 0o600);
}

#[test]
fn read_optional_returns_some_for_existing_file() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("present.txt");
    fs::write(&path, "content here").unwrap();

    assert_eq!(
        read_optional(&path).unwrap(),
        Some("content here".to_owned())
    );
}

#[test]
fn read_optional_returns_none_for_missing_path() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("does-not-exist.txt");

    assert_eq!(read_optional(&path).unwrap(), None);
}

#[test]
fn read_optional_round_trips_with_atomic_write() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("round.txt");
    let payload = "round trip payload\nwith newlines\n";

    atomic_write_0600(&path, payload.as_bytes()).unwrap();

    assert_eq!(read_optional(&path).unwrap(), Some(payload.to_owned()));
}

#[test]
fn copy_dir_recursive_copies_nested_tree() {
    let dir = tempdir().unwrap();
    let src = dir.path().join("src");
    let dst = dir.path().join("dst");

    fs::create_dir_all(src.join("nested").join("deeper")).unwrap();
    fs::write(src.join("top.txt"), b"top").unwrap();
    fs::write(src.join("nested").join("mid.txt"), b"mid").unwrap();
    fs::write(src.join("nested").join("deeper").join("low.txt"), b"low").unwrap();

    copy_dir_recursive(&src, &dst).unwrap();

    assert_eq!(fs::read(dst.join("top.txt")).unwrap(), b"top");
    assert_eq!(
        fs::read(dst.join("nested").join("mid.txt")).unwrap(),
        b"mid"
    );
    assert_eq!(
        fs::read(dst.join("nested").join("deeper").join("low.txt")).unwrap(),
        b"low"
    );
    assert!(dst.join("nested").join("deeper").is_dir());
}

#[test]
fn copy_dir_recursive_is_noop_when_src_missing() {
    let dir = tempdir().unwrap();
    let src = dir.path().join("missing-src");
    let dst = dir.path().join("dst");

    copy_dir_recursive(&src, &dst).unwrap();

    assert!(!dst.exists());
}

#[test]
fn copy_dir_recursive_errors_when_src_is_file() {
    let dir = tempdir().unwrap();
    let src = dir.path().join("a-file.txt");
    let dst = dir.path().join("dst");
    fs::write(&src, b"i am a file").unwrap();

    let err = copy_dir_recursive(&src, &dst).unwrap_err();

    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
}

#[test]
fn temp_path_for_is_sibling_with_prefixed_name() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("target.txt");

    let tmp = temp_path_for(&path);

    assert_ne!(tmp, path);
    assert_eq!(tmp.parent(), path.parent());

    let tmp_name = tmp.file_name().unwrap().to_string_lossy().into_owned();
    assert!(tmp_name.starts_with("target.txt"));
}

#[test]
fn create_dir_all_mode_creates_nested_dirs() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("x").join("y").join("z");

    create_dir_all_mode_0700(&path).unwrap();

    assert!(path.is_dir());
}

#[cfg(unix)]
#[test]
fn create_dir_all_mode_sets_mode_0700() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("secured");

    create_dir_all_mode_0700(&path).unwrap();

    assert_eq!(mode_of(&path), 0o700);
}
