//! `whoami` command: prints the authenticated identity.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::process::ExitCode;

use systemprompt_identifiers::SessionId;

use crate::context::BridgeContext;
use crate::stdio::diag;
use crate::{auth, config, stdio};

pub fn cmd_whoami(ctx: &BridgeContext) -> ExitCode {
    ctx.block_on(async {
        let cfg = match config::load() {
            Ok(cfg) => cfg,
            Err(e) => {
                diag(&e.to_string());
                return ExitCode::FAILURE;
            },
        };
        let gateway = config::gateway_url_or_default(&cfg);
        let out = match auth::acquire_bearer(&cfg, &SessionId::generate(), &ctx.http).await {
            Ok(out) => out,
            Err(e) => {
                let (code, message) = e.exit_report();

                diag(&message);

                return code;
            },
        };

        let client = ctx.gateway_client(gateway.clone());
        match client.fetch_whoami(out.token.expose()).await {
            Ok(value) => {
                match serde_json::to_string_pretty(&value) {
                    Ok(s) => stdio::print_line(&s),
                    Err(_) => stdio::print_line(&format!("{value:?}")),
                }
                ExitCode::SUCCESS
            },
            Err(e) => {
                diag(&format!("whoami failed: {e}"));
                ExitCode::from(3)
            },
        }
    })
}
