use std::process::ExitCode;

use crate::cli::output;
use crate::validate;

pub(crate) fn cmd_validate() -> ExitCode {
    let report = validate::run();
    output::print_str(&report.rendered());
    if report.any_failed {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}
