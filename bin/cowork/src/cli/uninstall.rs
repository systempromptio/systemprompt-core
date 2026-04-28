use std::process::ExitCode;

use crate::cli::args::has_flag;
use crate::install;

pub(crate) fn cmd_uninstall(args: &[String]) -> ExitCode {
    let purge = has_flag(args, "--purge");
    install::uninstall(purge)
}
