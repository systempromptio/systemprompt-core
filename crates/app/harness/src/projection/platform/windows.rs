//! Windows: a runtime file has no mode bits to set or to read back.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;
use std::{fs, io};

use systemprompt_models::feedback::receipts::ReadbackStatus;

use super::Platform;

pub(super) struct Windows;

impl Platform for Windows {
    fn set_mode(_path: &Path, _executable: bool) -> io::Result<()> {
        Ok(())
    }

    // Why: Windows has no mode bits; a plain file has nothing to verify and an
    // executable cannot be confirmed.
    fn mode_check(metadata: Option<&fs::Metadata>, executable: bool) -> ReadbackStatus {
        match metadata {
            None => ReadbackStatus::Mismatch,
            Some(_) if executable => ReadbackStatus::Unavailable,
            Some(_) => ReadbackStatus::Verified,
        }
    }
}
