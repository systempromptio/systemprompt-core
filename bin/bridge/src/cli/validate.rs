use std::process::ExitCode;

use crate::validate;

pub(crate) fn cmd_validate() -> ExitCode {
    let report = validate::run();
    print!("{}", report.rendered());
    if report.any_failed {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}
