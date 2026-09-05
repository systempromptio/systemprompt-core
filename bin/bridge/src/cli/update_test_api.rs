//! Delegating seam over the `update` command's private helpers so the
//! separate test workspace can drive their non-terminal arms.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::update::DownloadProgress;

#[must_use]
pub fn progress_reporter() -> Box<dyn Fn(DownloadProgress) + Send + Sync> {
    super::progress_reporter()
}

#[must_use]
pub fn confirm(version: &str) -> bool {
    super::confirm(version)
}
