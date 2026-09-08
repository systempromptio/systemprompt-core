//! `run` command: foreground bridge process with proxy and sync loop.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::process::ExitCode;

use systemprompt_identifiers::SessionId;

use crate::context::BridgeContext;
use crate::stdio::{diag, emit_json};
use crate::{auth, config};

pub(super) fn cmd_run(ctx: &BridgeContext) -> ExitCode {
    let cfg = match config::load() {
        Ok(cfg) => cfg,
        Err(e) => {
            diag(&e.to_string());
            return ExitCode::FAILURE;
        },
    };
    let session_id = SessionId::generate();
    let acquired = ctx.block_on(auth::acquire_bearer(&cfg, &session_id, &ctx.http));
    let out = match acquired {
        Ok(out) => out,
        Err(e) => {
            let (code, message) = e.exit_report();

            diag(&message);

            return code;
        },
    };
    if emit_json(&out).is_err() {
        return ExitCode::from(2);
    }
    ExitCode::SUCCESS
}
