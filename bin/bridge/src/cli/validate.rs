//! `validate` command: runs the self-diagnosis checks and prints the report.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::process::ExitCode;

use crate::context::BridgeContext;
use crate::{stdio, validate};

pub fn cmd_validate(ctx: &BridgeContext) -> ExitCode {
    let report = ctx.block_on(validate::run(ctx));
    stdio::print_str(&report.rendered());
    if report.any_failed {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}
