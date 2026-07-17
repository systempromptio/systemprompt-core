//! `uninstall` command: removes the bridge install and scheduled task.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::process::ExitCode;

use crate::cli::args::has_flag;
use crate::cli::output;
use crate::install;
use crate::obs::output::diag;

pub(super) fn cmd_uninstall(args: &[String]) -> ExitCode {
    let purge = has_flag(args, "--purge");
    match install::uninstall(purge) {
        Ok(summary) => {
            output::print_str(&install::render_uninstall_summary(&summary));
            ExitCode::SUCCESS
        },
        Err(err) => {
            diag(&err.to_string());
            install::InstallError::EXIT_CODE
        },
    }
}
