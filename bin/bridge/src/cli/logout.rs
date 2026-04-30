use std::process::ExitCode;

use crate::auth::setup;
use crate::cli::output;
use crate::obs::output::diag;

pub(crate) fn cmd_logout() -> ExitCode {
    match setup::logout() {
        Ok(paths) => {
            output::print_line("Removed PAT.");
            output::print_line(&format!("  config: {}", paths.config_file.display()));
            output::print_line(&format!("  secret: {}", paths.pat_file.display()));
            ExitCode::SUCCESS
        },
        Err(e) => {
            diag(&format!("logout failed: {e}"));
            ExitCode::from(1)
        },
    }
}
