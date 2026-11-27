use sp_secure::{SecureError, contained_path};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn contained_path_joins_safe_segments() -> TestResult {
    let root = tempfile::tempdir()?;
    let path = contained_path(root.path(), "skills/review/SKILL.md")?;
    assert_eq!(
        path,
        root.path().join("skills").join("review").join("SKILL.md")
    );
    Ok(())
}

#[test]
fn contained_path_rejects_escapes_and_windows_aliases() -> TestResult {
    let root = tempfile::tempdir()?;
    for bad in [
        "",
        "/etc/passwd",
        "../outside",
        "a/../../b",
        "a/./b",
        "a//b",
        "a/",
        "a\\b",
        "C:evil",
        "file.txt:stream",
        "con",
        "nul.txt",
        "COM1",
        "lpt9.log",
        "trailing.",
        "trailing ",
        "what?",
        "tab\tname",
    ] {
        let result = contained_path(root.path(), bad);
        assert!(
            matches!(result, Err(SecureError::Escapes { .. })),
            "{bad:?} was accepted"
        );
    }
    Ok(())
}

#[test]
fn contained_path_allows_lookalike_names() -> TestResult {
    let root = tempfile::tempdir()?;
    for good in [
        "console.md",
        "com10",
        "COM0",
        "lpt",
        ".hidden",
        "a..b",
        "notes.v2.txt",
    ] {
        assert_eq!(contained_path(root.path(), good)?, root.path().join(good));
    }
    Ok(())
}

#[test]
fn contained_path_rejects_a_file_as_parent() -> TestResult {
    let root = tempfile::tempdir()?;
    std::fs::write(root.path().join("file"), b"x")?;
    assert!(matches!(
        contained_path(root.path(), "file/child"),
        Err(SecureError::Escapes { .. })
    ));
    Ok(())
}

#[cfg(unix)]
#[test]
fn contained_path_refuses_symlinks() -> TestResult {
    let root = tempfile::tempdir()?;
    let outside = tempfile::tempdir()?;
    std::os::unix::fs::symlink(outside.path(), root.path().join("linked"))?;
    assert!(matches!(
        contained_path(root.path(), "linked/SKILL.md"),
        Err(SecureError::Link { .. })
    ));
    std::os::unix::fs::symlink(outside.path().join("f"), root.path().join("leaf"))?;
    assert!(matches!(
        contained_path(root.path(), "leaf"),
        Err(SecureError::Link { .. })
    ));
    Ok(())
}
