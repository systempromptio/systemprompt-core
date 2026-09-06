//! Delegating seam over the `proxy` command so the separate test workspace
//! can drive the arms that return before it blocks on Ctrl-C.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::process::ExitCode;

use crate::context::BridgeContext;

pub fn cmd_proxy(ctx: &BridgeContext) -> ExitCode {
    super::cmd_proxy(ctx)
}
