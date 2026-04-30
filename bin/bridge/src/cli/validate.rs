use std::process::ExitCode;

use crate::cli::output;
use crate::validate;

pub(crate) fn cmd_validate() -> ExitCode {
    let report = match crate::proxy::block_on(validate::run()) {
        Ok(r) => r,
        Err(e) => {
            output::eprint_str(&format!("runtime init failed: {e}\n"));
            return ExitCode::from(70);
        },
    };
    output::print_str(&report.rendered());
    if report.any_failed {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}
