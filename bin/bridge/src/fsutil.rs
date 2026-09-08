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

    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    #[cfg(not(target_os = "windows"))]
    let mut staged = tempfile::NamedTempFile::new_in(parent)?;
    #[cfg(target_os = "windows")]
    let reader = crate::windows_acl::current_sid()?;
    #[cfg(target_os = "windows")]
    let mut staged = if mode == 0o600 {
        tempfile::Builder::new().make_in(parent, |path| {
            crate::windows_acl::create_private(path, &reader)
        })?
    } else {
        tempfile::NamedTempFile::new_in(parent)?
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        staged
            .as_file()
            .set_permissions(fs::Permissions::from_mode(mode))?;
    }
    io::Write::write_all(&mut staged, bytes)?;
    staged.as_file().sync_all()?;
    staged.persist(path).map_err(|error| error.error)?;
    verify_contents(path, bytes)?;
    #[cfg(target_os = "windows")]
    if mode == 0o600 {
        crate::windows_acl::verify_private(&fs::File::open(path)?, &reader)?;
    }
    #[cfg(unix)]
    {
        verify_mode(path, mode)?;
        fs::File::open(parent)?.sync_all()?;
    }
    Ok(())
}

#[cfg(unix)]
fn verify_mode(path: &Path, mode: u32) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    if fs::metadata(path)?.permissions().mode() & 0o777 != mode {
        return Err(io::Error::other(format!(
            "{}: permissions did not read back as {mode:o}",
            path.display()
        )));
    }
    Ok(())
}

pub fn verify_contents(path: &Path, expected: &[u8]) -> io::Result<()> {
    let actual = fs::read(path)?;
    if actual != expected {
        return Err(io::Error::other(format!(
            "{}: contents did not match the requested write",
            path.display()
        )));
    }
    Ok(())
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
    #[cfg(target_os = "windows")]
    crate::windows_acl::protect_directory(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
        verify_mode(path, 0o700)?;
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

pub fn remove_verified(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => {},
        Err(e) if e.kind() == io::ErrorKind::NotFound => {},
        Err(e) => {
            return Err(io::Error::new(
                e.kind(),
                format!("remove {}: {e}", path.display()),
            ));
        },
    }
    match fs::symlink_metadata(path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
        Ok(_) => Err(io::Error::other(format!(
            "{} still exists after deletion",
            path.display()
        ))),
    }
}

#[cfg(target_os = "windows")]
pub fn atomic_write_for_reader(path: &Path, bytes: &[u8], reader: &str) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("private file has no parent"))?;
    let mut staged = tempfile::Builder::new().make_in(parent, |path| {
        crate::windows_acl::create_private(path, reader)
    })?;
    io::Write::write_all(&mut staged, bytes)?;
    staged.as_file().sync_all()?;
    staged.persist(path).map_err(|e| e.error)?;
    verify_contents(path, bytes)?;
    crate::windows_acl::verify_private(&fs::File::open(path)?, reader)
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use]
pub struct FileReceipt {
    path: std::path::PathBuf,
    sha256: String,
}
impl FileReceipt {
    pub fn verify(path: &Path, expected: &[u8]) -> io::Result<Self> {
        verify_contents(path, expected)?;
        Ok(Self {
            path: path.to_path_buf(),
            sha256: crate::hash::sha256_hex(expected),
        })
    }
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn verify_directory_write(path: &Path) -> io::Result<()> {
    let mut probe = tempfile::NamedTempFile::new_in(path)?;
    let marker = uuid::Uuid::new_v4().to_string();
    io::Write::write_all(&mut probe, marker.as_bytes())?;
    probe.as_file().sync_all()?;
    verify_contents(probe.path(), marker.as_bytes())?;
    let name = probe.path().to_path_buf();
    probe.close()?;
    match fs::symlink_metadata(&name) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
        Ok(_) => Err(io::Error::other(format!(
            "{}: access probe removal did not land",
            name.display()
        ))),
    }
}

// Why: temporary probes and staging trees are cleaned up on a best-effort
// basis; a leftover cannot change the installed result, but it must not be
// silent either, because a directory that cannot be removed is usually the
// first sign of a permission problem the next step will hit.
pub fn remove_leftover_file(path: &Path) {
    if let Err(e) = fs::remove_file(path)
        && e.kind() != io::ErrorKind::NotFound
    {
        tracing::warn!(error = %e, path = %path.display(), "leftover file was not removed");
    }
}

pub fn remove_leftover_dir(path: &Path) {
    if let Err(e) = fs::remove_dir_all(path)
        && e.kind() != io::ErrorKind::NotFound
    {
        tracing::warn!(error = %e, path = %path.display(), "leftover directory was not removed");
    }
}
