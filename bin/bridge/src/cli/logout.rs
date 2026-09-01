//! `logout` command: clears stored credentials.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::process::ExitCode;

use crate::auth::setup;
use crate::stdio;
use crate::stdio::diag;

pub fn cmd_logout() -> ExitCode {
    match setup::logout() {
        Ok(paths) => {
            stdio::print_line("Removed PAT.");
            stdio::print_line(&format!("  config: {}", paths.config_file.display()));
            stdio::print_line(&format!("  secret: {}", paths.pat_file.display()));
            ExitCode::SUCCESS
        },
        Err(e) => {
            diag(&format!("logout failed: {e}"));
            ExitCode::from(1)
        },
    }
}
