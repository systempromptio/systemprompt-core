use std::process::ExitCode;

use crate::auth::setup;
use crate::obs::output::diag;

pub(crate) fn cmd_clean() -> ExitCode {
    match setup::clean() {
        Ok(report) => {
            if report.config_removed {
                println!("Removed config: {}", report.paths.config_file.display());
            } else {
                println!(
                    "No config at {} (already clean)",
                    report.paths.config_file.display()
                );
            }
            if report.pat_removed {
                println!("Removed PAT:    {}", report.paths.pat_file.display());
            } else {
                println!(
                    "No PAT at    {} (already clean)",
                    report.paths.pat_file.display()
                );
            }
            println!("Token cache cleared.");
            println!("Cowork is back to a fresh splash on next launch.");
            ExitCode::SUCCESS
        },
        Err(e) => {
            diag(&format!("clean failed: {e}"));
            ExitCode::from(1)
        },
    }
}
