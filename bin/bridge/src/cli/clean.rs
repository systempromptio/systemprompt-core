use std::process::ExitCode;

use crate::auth::setup;
use crate::cli::output;
use crate::obs::output::diag;

pub(crate) fn cmd_clean() -> ExitCode {
    match setup::clean() {
        Ok(report) => {
            if report.config_removed {
                output::print_line(&format!(
                    "Removed config: {}",
                    report.paths.config_file.display()
                ));
            } else {
                output::print_line(&format!(
                    "No config at {} (already clean)",
                    report.paths.config_file.display()
                ));
            }
            if report.pat_removed {
                output::print_line(&format!(
                    "Removed PAT:    {}",
                    report.paths.pat_file.display()
                ));
            } else {
                output::print_line(&format!(
                    "No PAT at    {} (already clean)",
                    report.paths.pat_file.display()
                ));
            }
            output::print_line("Token cache cleared.");
            output::print_line("Cowork is back to a fresh splash on next launch.");
            ExitCode::SUCCESS
        },
        Err(e) => {
            diag(&format!("clean failed: {e}"));
            ExitCode::from(1)
        },
    }
}
