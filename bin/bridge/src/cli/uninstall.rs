//! `uninstall` command: removes the bridge install and scheduled task.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::process::ExitCode;

use crate::cli::args::has_flag;
use crate::context::BridgeContext;
use crate::stdio::diag;
use crate::{install, stdio};

pub(super) fn cmd_uninstall(ctx: &BridgeContext, args: &[String]) -> ExitCode {
    let purge = has_flag(args, "--purge");
    match install::uninstall(purge, ctx) {
        Ok(summary) => {
            crate::integration::uninstall::clear_hosts();
            stdio::print_str(&install::render_uninstall_summary(&summary));
            ExitCode::SUCCESS
        },
        Err(err) => {
            diag(&err.to_string());
            install::InstallError::EXIT_CODE
        },
    }
}
