//! The profile a host generates before it is installed: a private file under
//! the brand's temp directory that exists only between `generate_profile`
//! and `install_profile`.
//!
//! The body carries a credential derived from the loopback secret, so it is
//! written 0600 and removed once the installer has consumed it. A profile the
//! OS installs asynchronously (a macOS `.mobileconfig` opened in System
//! Settings) is the one case that outlives the call; it stays private.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn write(stem: &str, extension: &str, body: &[u8]) -> io::Result<PathBuf> {
    let dir = std::env::temp_dir().join(crate::brand::brand().working_dir_name);
    crate::fsutil::create_dir_all_mode_0700(&dir)?;
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let path = dir.join(format!("{stem}-{}-{nanos}{extension}", std::process::id()));
    crate::fsutil::atomic_write_0600(&path, body)?;
    Ok(path)
}

/// The `PayloadUUID` / profile identifier pair a generated profile carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileUuids {
    pub payload: String,
    pub profile: String,
}

#[must_use]
pub fn profile_uuids() -> ProfileUuids {
    ProfileUuids {
        payload: uuid::Uuid::new_v4().to_string().to_ascii_uppercase(),
        profile: uuid::Uuid::new_v4().to_string().to_ascii_uppercase(),
    }
}

pub fn consume(generated_path: &str) -> io::Result<()> {
    crate::fsutil::remove_verified(Path::new(generated_path))
}
