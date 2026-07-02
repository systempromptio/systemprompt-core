//! Bridge filesystem helpers: atomic 0600 writes and optional reads.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{fs, io};

pub fn atomic_write_0600(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
        && !parent.exists()
    {
        create_dir_all_mode_0700(parent)?;
    }

    let tmp = temp_path_for(path);

    {
        let mut opts = fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            // 0600 at create() closes the TOCTOU window between write and chmod.
            opts.mode(0o600);
        }
        let mut file = opts.open(&tmp)?;
        io::Write::write_all(&mut file, bytes)?;
        file.sync_all()?;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // Guards a pre-existing temp with looser perms when OpenOptions::mode was
        // ignored.
        _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600));
    }

    match fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(e) => {
            // Why: best-effort temp cleanup; the rename error is the failure to report.
            _ = fs::remove_file(&tmp);
            Err(e)
        },
    }
}

pub fn read_optional(path: &Path) -> io::Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(s) => Ok(Some(s)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

pub fn create_dir_all_mode_0700(path: &Path) -> io::Result<()> {
    fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // Why: best-effort tightening; the directory exists either way.
        _ = fs::set_permissions(path, fs::Permissions::from_mode(0o700));
    }
    Ok(())
}

pub fn temp_path_for(path: &Path) -> std::path::PathBuf {
    // pid+nanos suffix avoids lost writes when two bridge processes race the same
    // target.
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let pid = std::process::id();
    let suffix = format!("tmp.{pid}.{nanos}");
    let mut name = path
        .file_name()
        .map(std::ffi::OsString::from)
        .unwrap_or_default();
    name.push(".");
    name.push(suffix);
    match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.join(name),
        _ => std::path::PathBuf::from(name),
    }
}
