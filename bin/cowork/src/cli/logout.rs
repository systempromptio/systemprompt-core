use std::process::ExitCode;

use crate::obs::output::diag;
use crate::setup;

pub(crate) fn cmd_logout() -> ExitCode {
    match setup::logout() {
        Ok(paths) => {
            println!("Removed PAT.");
            println!("  config: {}", paths.config_file.display());
            println!("  secret: {}", paths.pat_file.display());
            ExitCode::SUCCESS
        },
        Err(e) => {
            diag(&format!("logout failed: {e}"));
            ExitCode::from(1)
        },
    }
}
