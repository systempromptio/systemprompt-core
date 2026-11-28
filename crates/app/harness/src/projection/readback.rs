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
use crate::projection::platform;

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
        mode_check: platform::mode_check(metadata.as_ref(), planned.executable),
    })
}

pub(crate) fn set_mode(path: &Path, executable: bool) -> Result<(), MaterializeError> {
    platform::set_mode(path, executable).map_err(|source| MaterializeError::Io {
        op: "set the mode of",
        path: path.to_path_buf(),
        source,
    })
}
