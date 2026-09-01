//! Bridge filesystem helpers: atomic mode-pinned writes and optional reads.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{fs, io};

pub fn atomic_write_0600(path: &Path, bytes: &[u8]) -> io::Result<()> {
    atomic_write_with_mode(path, bytes, 0o600)
}

// Why: a managed config the host reads as an unprivileged user must stay
// world-readable; 0600 would lock the host out of its own policy.
pub fn atomic_write_0644(path: &Path, bytes: &[u8]) -> io::Result<()> {
    atomic_write_with_mode(path, bytes, 0o644)
}

fn atomic_write_with_mode(path: &Path, bytes: &[u8], mode: u32) -> io::Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
        && !parent.exists()
    {
        if mode == 0o600 {
            create_dir_all_mode_0700(parent)?;
        } else {
            fs::create_dir_all(parent)?;
        }
    }

    let tmp = temp_path_for(path);

    {
        let mut opts = fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            // Why: the mode at create() closes the TOCTOU window between write
            // and chmod.
            opts.mode(mode);
        }
        let mut file = opts.open(&tmp)?;
        io::Write::write_all(&mut file, bytes)?;
        file.sync_all()?;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // Why: guards a pre-existing temp with different perms when
        // OpenOptions::mode was ignored.
        _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(mode));
    }
    #[cfg(not(unix))]
    {
        let _ = mode;
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

pub fn expand_tilde(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/")
        && let Some(home) = crate::basedirs::home_dir()
    {
        return home.join(rest).to_string_lossy().into_owned();
    }
    path.to_owned()
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
