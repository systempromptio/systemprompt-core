//! Unix: a settled path is compared as written.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

use super::Platform;

pub(super) struct Unix;

impl Platform for Unix {
    fn comparable(settled: PathBuf) -> PathBuf {
        settled
    }
}
