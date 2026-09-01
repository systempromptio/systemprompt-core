//! `clean` command: removes cached sync state and materialized plugins.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::process::ExitCode;

use crate::auth::setup;
use crate::stdio;
use crate::stdio::diag;

pub fn cmd_clean() -> ExitCode {
    match setup::clean() {
        Ok(report) => {
            if report.config_removed {
                stdio::print_line(&format!(
                    "Removed config: {}",
                    report.paths.config_file.display()
                ));
            } else {
                stdio::print_line(&format!(
                    "No config at {} (already clean)",
                    report.paths.config_file.display()
                ));
            }
            if report.pat_removed {
                stdio::print_line(&format!(
                    "Removed PAT:    {}",
                    report.paths.pat_file.display()
                ));
            } else {
                stdio::print_line(&format!(
                    "No PAT at    {} (already clean)",
                    report.paths.pat_file.display()
                ));
            }
            stdio::print_line("Token cache cleared.");
            if report.oauth_creds_removed {
                stdio::print_line("OAuth client credentials cleared.");
            } else {
                stdio::print_line("No OAuth client credentials (already clean).");
            }
            stdio::print_line("Bridge is back to a fresh splash on next launch.");
            ExitCode::SUCCESS
        },
        Err(e) => {
            diag(&format!("clean failed: {e}"));
            ExitCode::from(1)
        },
    }
}
