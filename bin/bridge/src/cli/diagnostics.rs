//! `diagnostics` command: build/version info and environment checks.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::process::ExitCode;

use crate::stdio;

pub use crate::buildinfo::{
    BUILD_TIMESTAMP, GIT_BRANCH, GIT_COMMIT_DATE, GIT_SHA, render, short_sha,
};

pub fn cmd_diagnostics() -> ExitCode {
    stdio::print_str(&render());
    ExitCode::SUCCESS
}
