//! Ownership of the configuration directory relative to the running account.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DirOwner {
    Absent,
    Current,
    Foreign { owner: String },
}

#[cfg(target_os = "windows")]
pub fn config_dir_owner(dir: &Path) -> io::Result<DirOwner> {
    if !dir.try_exists()? {
        return Ok(DirOwner::Absent);
    }
    let owner = crate::windows_acl::owner_sid(dir)?;
    if owner == crate::windows_acl::current_sid()? {
        Ok(DirOwner::Current)
    } else {
        Ok(DirOwner::Foreign { owner })
    }
}

#[cfg(not(target_os = "windows"))]
pub fn config_dir_owner(dir: &Path) -> io::Result<DirOwner> {
    if dir.try_exists()? {
        Ok(DirOwner::Current)
    } else {
        Ok(DirOwner::Absent)
    }
}
