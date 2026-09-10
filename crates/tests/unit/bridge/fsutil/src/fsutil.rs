use std::fs;

use systemprompt_bridge::fsutil::{
    atomic_write_0600, create_dir_all_mode_0700, read_optional, temp_path_for,
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

#[cfg(windows)]
mod windows_private_files {
    use std::fs;
    use std::io::ErrorKind;

    use systemprompt_bridge::fsutil::{atomic_write_0600, create_dir_all_mode_0700, read_private};
    use tempfile::tempdir;

    fn icacls(path: &std::path::Path) -> String {
        let output = std::process::Command::new("icacls")
            .arg(path)
            .output()
            .expect("icacls runs");
        String::from_utf8_lossy(&output.stdout).into_owned()
    }

    // Why: a CI runner's temp files carry explicit entries as well as
    // inherited ones, so dropping inheritance alone leaves them readable;
    // every listed account is removed to reproduce the empty DACL.
    fn strip_every_ace(path: &std::path::Path) {
        let status = std::process::Command::new("icacls")
            .arg(path)
            .arg("/inheritance:r")
            .status()
            .expect("icacls runs");
        assert!(status.success(), "icacls /inheritance:r failed");
        let shown = path.display().to_string();
        let accounts: Vec<String> = icacls(path)
            .lines()
            .filter_map(|line| line.split_once(":(").map(|(account, _)| account))
            .map(|account| {
                account
                    .trim()
                    .strip_prefix(&shown)
                    .unwrap_or(account)
                    .trim()
                    .to_owned()
            })
            .filter(|account| !account.is_empty())
            .collect();
        for account in accounts {
            let status = std::process::Command::new("icacls")
                .arg(path)
                .arg("/remove:g")
                .arg(&account)
                .status()
                .expect("icacls runs");
            assert!(status.success(), "icacls /remove:g {account} failed");
        }
    }

    #[test]
    fn protect_directory_keeps_existing_inherited_files_readable() {
        let dir = tempdir().unwrap();
        let key = dir.path().join("bridge-loopback.key");
        fs::write(&key, b"minted-by-an-older-release").unwrap();

        create_dir_all_mode_0700(dir.path()).unwrap();

        assert_eq!(fs::read(&key).unwrap(), b"minted-by-an-older-release");
    }

    #[test]
    fn protect_directory_makes_default_security_children_private() {
        let dir = tempdir().unwrap();
        create_dir_all_mode_0700(dir.path()).unwrap();
        let child = dir.path().join("plain.txt");
        fs::write(&child, b"x").unwrap();

        let listing = icacls(&child);

        assert_eq!(fs::read(&child).unwrap(), b"x");
        assert!(listing.contains("NT AUTHORITY\\SYSTEM"), "{listing}");
        assert!(listing.contains("BUILTIN\\Administrators"), "{listing}");
        assert!(!listing.contains("BUILTIN\\Users"), "{listing}");
        assert!(!listing.contains("Everyone"), "{listing}");
    }

    #[test]
    fn read_private_repairs_owner_file_with_empty_dacl() {
        let dir = tempdir().unwrap();
        let key = dir.path().join("bridge-install.id");
        fs::write(&key, b"yqasnH1BwF8").unwrap();
        strip_every_ace(&key);
        assert_eq!(
            fs::read(&key).map_err(|e| e.kind()),
            Err(ErrorKind::PermissionDenied)
        );

        assert_eq!(read_private(&key).unwrap(), b"yqasnH1BwF8");

        assert_eq!(fs::read(&key).unwrap(), b"yqasnH1BwF8");
        let listing = icacls(&key);
        assert!(listing.contains("NT AUTHORITY\\SYSTEM"), "{listing}");
        assert!(!listing.contains("(I)"), "{listing}");
    }

    #[test]
    fn atomic_write_0600_survives_directory_reprotection() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("private.txt");
        atomic_write_0600(&path, b"secret").unwrap();

        create_dir_all_mode_0700(dir.path()).unwrap();

        assert_eq!(read_private(&path).unwrap(), b"secret");
        assert!(!icacls(&path).contains("(I)"));
    }
}

#[test]
fn read_private_reads_a_plain_file() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("plain.txt");
    fs::write(&path, b"contents").unwrap();
    assert_eq!(
        systemprompt_bridge::fsutil::read_private(&path).unwrap(),
        b"contents"
    );
}

#[test]
fn read_private_reports_a_missing_file() {
    let dir = tempdir().unwrap();
    assert_eq!(
        systemprompt_bridge::fsutil::read_private(&dir.path().join("absent")).map_err(|e| e.kind()),
        Err(std::io::ErrorKind::NotFound)
    );
}
