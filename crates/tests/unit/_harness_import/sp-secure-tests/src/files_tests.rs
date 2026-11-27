use sp_secure::{
    Access, WriteOutcome, read_optional, remove_if_exists, write_if_changed, write_private,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn write_if_changed_writes_only_on_change() -> TestResult {
    let root = tempfile::tempdir()?;
    let path = root.path().join("nested").join("SKILL.md");
    assert_eq!(
        write_if_changed(&path, b"one", Access::Shared)?,
        WriteOutcome::Written
    );
    assert_eq!(
        write_if_changed(&path, b"one", Access::Shared)?,
        WriteOutcome::Unchanged
    );
    assert_eq!(
        write_if_changed(&path, b"two", Access::Shared)?,
        WriteOutcome::Written
    );
    assert_eq!(std::fs::read(&path)?, b"two");
    let leftovers = std::fs::read_dir(path.parent().ok_or("no parent")?)?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
        .count();
    assert_eq!(leftovers, 0);
    Ok(())
}

#[test]
fn write_private_round_trips_and_replaces() -> TestResult {
    let root = tempfile::tempdir()?;
    let path = root.path().join("private").join("accounts.json");
    write_private(&path, b"{}")?;
    write_private(&path, b"{\"a\":1}")?;
    assert_eq!(std::fs::read(&path)?, b"{\"a\":1}");
    Ok(())
}

#[cfg(unix)]
#[test]
fn write_private_is_owner_only() -> TestResult {
    use std::os::unix::fs::PermissionsExt as _;
    let root = tempfile::tempdir()?;
    let dir = root.path().join("private");
    let path = dir.join("accounts.json");
    write_private(&path, b"{}")?;
    assert_eq!(std::fs::read(&path)?, b"{}");
    assert_eq!(
        std::fs::metadata(&path)?.permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(std::fs::metadata(&dir)?.permissions().mode() & 0o777, 0o700);
    Ok(())
}

#[cfg(unix)]
#[test]
fn write_if_changed_restores_lost_private_mode() -> TestResult {
    use std::os::unix::fs::PermissionsExt as _;
    let root = tempfile::tempdir()?;
    let path = root.path().join("secret");
    write_private(&path, b"x")?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644))?;
    assert_eq!(
        write_if_changed(&path, b"x", Access::Private)?,
        WriteOutcome::Written
    );
    assert_eq!(
        std::fs::metadata(&path)?.permissions().mode() & 0o777,
        0o600
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn writes_refuse_a_symlinked_target() -> TestResult {
    use sp_secure::SecureError;
    let root = tempfile::tempdir()?;
    let outside = tempfile::tempdir()?;
    let target = outside.path().join("victim");
    std::fs::write(&target, b"original")?;
    let link = root.path().join("link");
    std::os::unix::fs::symlink(&target, &link)?;
    assert!(matches!(
        write_private(&link, b"x"),
        Err(SecureError::Link { .. })
    ));
    assert!(matches!(
        write_if_changed(&link, b"x", Access::Shared),
        Err(SecureError::Link { .. })
    ));
    assert!(matches!(
        read_optional(&link),
        Err(SecureError::Link { .. })
    ));
    assert_eq!(std::fs::read(&target)?, b"original");
    Ok(())
}

#[test]
fn remove_if_exists_reports_presence() -> TestResult {
    let root = tempfile::tempdir()?;
    let path = root.path().join("gone");
    std::fs::write(&path, b"x")?;
    assert!(remove_if_exists(&path)?);
    assert!(!remove_if_exists(&path)?);
    assert!(!path.exists());
    Ok(())
}

#[test]
fn read_optional_is_none_when_missing() -> TestResult {
    let root = tempfile::tempdir()?;
    assert!(read_optional(&root.path().join("missing"))?.is_none());
    Ok(())
}
