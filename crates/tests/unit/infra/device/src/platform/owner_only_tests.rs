//! Owner-only files and directories on macOS and Linux: mode, ownership and
//! link refusal.

use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};

use systemprompt_device::platform::DataRoot;
use systemprompt_device::secure::{
    Access, SecureError, WriteOutcome, ensure_private_dir, read_optional, write_if_changed,
    write_private,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn a_private_file_is_mode_0600_and_owned_by_the_user_who_wrote_it() -> TestResult {
    let root = tempfile::tempdir()?;
    let path = root.path().join("state").join("accounts.json");
    write_private(&path, b"{}")?;
    let metadata = std::fs::symlink_metadata(&path)?;
    assert!(metadata.is_file());
    assert_eq!(metadata.mode() & 0o777, 0o600);
    assert_eq!(metadata.uid(), std::fs::metadata(root.path())?.uid());
    assert_eq!(read_optional(&path)?.as_deref(), Some(b"{}".as_slice()));
    Ok(())
}

#[test]
fn an_unchanged_private_file_with_a_wider_mode_is_written_again() -> TestResult {
    let root = tempfile::tempdir()?;
    let path = root.path().join("secret");
    write_private(&path, b"x")?;
    assert_eq!(
        write_if_changed(&path, b"x", Access::Private)?,
        WriteOutcome::Unchanged
    );
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o640))?;
    assert_eq!(
        write_if_changed(&path, b"x", Access::Private)?,
        WriteOutcome::Written
    );
    assert_eq!(std::fs::metadata(&path)?.mode() & 0o777, 0o600);
    Ok(())
}

#[test]
fn every_directory_of_a_data_root_is_mode_0700() -> TestResult {
    let base = tempfile::tempdir()?;
    let layout = DataRoot::at(base.path().join("Example").join("Product"));
    layout.ensure()?;
    for dir in [
        layout.root().to_path_buf(),
        layout.data(),
        layout.config(),
        layout.cache(),
        layout.state(),
        layout.tmp(),
    ] {
        assert_eq!(
            std::fs::metadata(&dir)?.mode() & 0o777,
            0o700,
            "{}",
            dir.display()
        );
    }
    Ok(())
}

#[test]
fn a_private_directory_is_never_a_symlink() -> TestResult {
    let root = tempfile::tempdir()?;
    let real = root.path().join("real");
    std::fs::create_dir(&real)?;
    std::fs::set_permissions(&real, std::fs::Permissions::from_mode(0o755))?;
    let link = root.path().join("link");
    std::os::unix::fs::symlink(&real, &link)?;
    assert!(matches!(
        ensure_private_dir(&link),
        Err(SecureError::Link { .. })
    ));
    assert_eq!(std::fs::metadata(&real)?.mode() & 0o777, 0o755);
    Ok(())
}

#[test]
fn a_dangling_symlink_is_refused_not_read_as_missing() -> TestResult {
    let root = tempfile::tempdir()?;
    let link = root.path().join("link");
    std::os::unix::fs::symlink(root.path().join("nowhere"), &link)?;
    assert!(matches!(
        read_optional(&link),
        Err(SecureError::Link { .. })
    ));
    assert!(matches!(
        write_private(&link, b"x"),
        Err(SecureError::Link { .. })
    ));
    assert!(!root.path().join("nowhere").exists());
    Ok(())
}
