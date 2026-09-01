//! `validate` command: runs the self-diagnosis checks and prints the report.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::process::ExitCode;

use crate::{stdio, validate};

pub fn cmd_validate() -> ExitCode {
    let report = match crate::proxy::block_on(validate::run()) {
        Ok(r) => r,
        Err(e) => {
            stdio::eprint_str(&format!("runtime init failed: {e}\n"));
            return ExitCode::from(70);
        },
    };
    stdio::print_str(&report.rendered());
    if report.any_failed {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}
