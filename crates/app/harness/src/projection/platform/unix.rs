//! Unix: a runtime file has mode 0644, or 0755 when executable.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;
use std::{fs, io};

use systemprompt_models::feedback::receipts::ReadbackStatus;

use super::Platform;

pub(super) struct Unix;

const fn expected_mode(executable: bool) -> u32 {
    if executable { 0o755 } else { 0o644 }
}

impl Platform for Unix {
    fn set_mode(path: &Path, executable: bool) -> io::Result<()> {
        fs::set_permissions(path, fs::Permissions::from_mode(expected_mode(executable)))
    }

    fn mode_check(metadata: Option<&fs::Metadata>, executable: bool) -> ReadbackStatus {
        match metadata {
            Some(metadata)
                if metadata.permissions().mode() & 0o7777 == expected_mode(executable) =>
            {
                ReadbackStatus::Verified
            },
            _ => ReadbackStatus::Mismatch,
        }
    }
}
