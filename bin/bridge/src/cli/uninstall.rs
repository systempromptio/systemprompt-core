//! `uninstall` command: removes the bridge install and scheduled task, or with
//! `--host <id>` just that host's bridge-owned settings.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::process::ExitCode;

use crate::cli::args::{has_flag, parse_multi_flag};
use crate::context::BridgeContext;
use crate::integration::enrol::{self, Selection};
use crate::stdio::diag;
use crate::{install, stdio};

pub(super) fn cmd_uninstall(ctx: &BridgeContext, args: &[String]) -> ExitCode {
    // Why: naming hosts scopes the command to them. Removing the whole bridge
    // because the operator asked to un-enrol one client would be a far larger
    // act than the words on the line.
    let hosts = parse_multi_flag(args, "--host");
    if !hosts.is_empty() {
        return remove_hosts(&Selection::Ids(hosts));
    }
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

fn remove_hosts(selection: &Selection) -> ExitCode {
    match enrol::remove_host_profiles(selection) {
        Ok(reports) => {
            stdio::print_str(&enrol::render(&reports));
            if reports.iter().any(enrol::Report::is_failure) {
                ExitCode::from(1)
            } else {
                ExitCode::SUCCESS
            }
        },
        Err(msg) => {
            diag(&msg);
            ExitCode::from(64)
        },
    }
}
