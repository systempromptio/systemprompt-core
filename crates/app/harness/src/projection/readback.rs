//! Reading a written runtime file back for the receipt: its digest, size and
//! mode, checked against the plan. Files are written with mode 0644, or 0755
//! when executable, where the platform has modes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::path::Path;

use systemprompt_device::protocol::names::sha256_hex;
use systemprompt_models::feedback::ContentDigest;
use systemprompt_models::feedback::receipts::{
    InstallationPlanFile, ReadbackStatus, RuntimeFileReadback,
};

use crate::projection::error::MaterializeError;

pub(crate) fn read_back(
    path: &Path,
    planned: &InstallationPlanFile,
) -> Result<RuntimeFileReadback, MaterializeError> {
    let metadata = fs::symlink_metadata(path)
        .ok()
        .filter(fs::Metadata::is_file);
    let bytes = match &metadata {
        Some(_) => fs::read(path).map_err(|source| MaterializeError::Io {
            op: "read back",
            path: path.to_path_buf(),
            source,
        })?,
        None => Vec::new(),
    };
    let digest = ContentDigest::try_from(sha256_hex(&bytes)).map_err(|_invalid| {
        MaterializeError::Digest {
            path: planned.path.clone(),
        }
    })?;
    let content_check = if metadata.is_some() && bytes == planned.bytes {
        ReadbackStatus::Verified
    } else {
        ReadbackStatus::Mismatch
    };
    Ok(RuntimeFileReadback {
        path: planned.path.clone(),
        digest,
        bytes: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
        executable: planned.executable,
        content_check,
        mode_check: mode_check(metadata.as_ref(), planned.executable),
    })
}

#[cfg(unix)]
const fn expected_mode(executable: bool) -> u32 {
    if executable { 0o755 } else { 0o644 }
}

#[cfg(unix)]
pub(crate) fn set_mode(path: &Path, executable: bool) -> Result<(), MaterializeError> {
    use std::os::unix::fs::PermissionsExt as _;
    fs::set_permissions(path, fs::Permissions::from_mode(expected_mode(executable))).map_err(
        |source| MaterializeError::Io {
            op: "set the mode of",
            path: path.to_path_buf(),
            source,
        },
    )
}

#[cfg(unix)]
fn mode_check(metadata: Option<&fs::Metadata>, executable: bool) -> ReadbackStatus {
    use std::os::unix::fs::PermissionsExt as _;
    match metadata {
        Some(metadata) if metadata.permissions().mode() & 0o7777 == expected_mode(executable) => {
            ReadbackStatus::Verified
        },
        _ => ReadbackStatus::Mismatch,
    }
}

// Why: Windows has no mode bits; a plain file has nothing to verify and an
// executable cannot be confirmed.
#[cfg(not(unix))]
const fn mode_check(metadata: Option<&fs::Metadata>, executable: bool) -> ReadbackStatus {
    match metadata {
        None => ReadbackStatus::Mismatch,
        Some(_) if executable => ReadbackStatus::Unavailable,
        Some(_) => ReadbackStatus::Verified,
    }
}
