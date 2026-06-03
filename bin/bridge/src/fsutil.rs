//! Canonical bridge filesystem helpers: atomic 0600 writes, recursive copies,
//! and optional reads.

#![allow(dead_code, reason = "helpers not yet wired into all call sites")]

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{fs, io};

pub(crate) fn atomic_write_0600(path: &Path, bytes: &[u8]) -> io::Result<()> {
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
            // Why: 0600 perms set at create() close the TOCTOU window where a
            // reader could open the temp file between write and chmod.
            opts.mode(0o600);
        }
        let mut file = opts.open(&tmp)?;
        io::Write::write_all(&mut file, bytes)?;
        file.sync_all()?;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // Why: belt-and-braces in case the file pre-existed with looser perms
        // and OpenOptions::mode was ignored on this platform/fs.
        _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600));
    }

    match fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(e) => {
            _ = fs::remove_file(&tmp);
            Err(e)
        },
    }
}

pub(crate) fn copy_dir_recursive(src: &Path, dst: &Path) -> io::Result<()> {
    if !src.exists() {
        return Ok(());
    }
    if !src.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{} is not a directory", src.display()),
        ));
    }
    if !dst.exists() {
        fs::create_dir_all(dst)?;
    }

    let mut stack = vec![src.to_path_buf()];
    while let Some(current) = stack.pop() {
        for entry in fs::read_dir(&current)? {
            let entry = entry?;
            let entry_path = entry.path();
            let metadata = fs::symlink_metadata(&entry_path)?;
            let rel = entry_path
                .strip_prefix(src)
                .map_err(|e| io::Error::other(e.to_string()))?;
            let target = dst.join(rel);

            if metadata.is_dir() {
                if !target.exists() {
                    fs::create_dir_all(&target)?;
                }
                stack.push(entry_path);
            } else if metadata.is_file() {
                if let Some(parent) = target.parent()
                    && !parent.exists()
                {
                    fs::create_dir_all(parent)?;
                }
                fs::copy(&entry_path, &target)?;
            }
        }
    }
    Ok(())
}

pub(crate) fn read_optional(path: &Path) -> io::Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(s) => Ok(Some(s)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

fn create_dir_all_mode_0700(path: &Path) -> io::Result<()> {
    fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        _ = fs::set_permissions(path, fs::Permissions::from_mode(0o700));
    }
    Ok(())
}

fn temp_path_for(path: &Path) -> std::path::PathBuf {
    // Why: rand suffix avoids collisions when two bridge processes race on the
    // same target path; pure ".tmp" lost writes under contention.
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
