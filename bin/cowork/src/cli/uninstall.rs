use std::process::ExitCode;

use crate::cli::args::has_flag;
use crate::install;

pub(crate) fn cmd_uninstall(args: &[String]) -> ExitCode {
    let purge = has_flag(args, "--purge");
    match install::uninstall(purge) {
        Ok(summary) => {
            print!("{}", install::render_uninstall_summary(&summary));
            ExitCode::SUCCESS
        },
        Err(code) => code,
    }
}
