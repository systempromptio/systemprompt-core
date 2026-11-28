//! Windows: a settled path is compared in lower case.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

use super::Platform;

pub(super) struct Windows;

impl Platform for Windows {
    fn comparable(settled: PathBuf) -> PathBuf {
        PathBuf::from(settled.to_string_lossy().to_lowercase())
    }
}
