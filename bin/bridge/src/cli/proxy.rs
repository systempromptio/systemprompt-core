//! `proxy` command: runs the loopback inference proxy standalone.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::process::ExitCode;
use std::sync::mpsc::channel;

use crate::cli::output;
use crate::obs::output::diag;

pub(super) fn cmd_proxy() -> ExitCode {
    match crate::proxy::start_default() {
        crate::proxy::StartOutcome::Started(_) => {},
        // Why: our own proxy already serving is the outcome this command wants,
        // not a failure. Starting a second one would only split the traffic.
        crate::proxy::StartOutcome::AlreadyRunning {
            port,
            pid,
            config_dir,
        } => {
            output::print_str(&format!(
                "{bin} proxy is already running on 127.0.0.1:{port} (pid {pid}, config \
                 {config_dir}) — nothing to do.\n",
                bin = crate::brand::brand().binary_name,
            ));
            return ExitCode::SUCCESS;
        },
        crate::proxy::StartOutcome::Failed { tried, last_error } => {
            diag(&format!(
                "proxy: failed to start; tried ports {tried:?}: {last_error}"
            ));
            return ExitCode::from(1);
        },
    }

    let origin = crate::proxy::loopback_origin();
    let secret = match crate::proxy::secret::for_profile() {
        Ok(s) => s.into_inner(),
        Err(e) => {
            diag(&format!(
                "proxy: started but loopback secret unavailable: {e}"
            ));
            return ExitCode::from(1);
        },
    };

    output::print_str(&format!(
        "{bin} proxy listening on {origin}\n\
         \n\
         Point an Anthropic-API client (Claude Code, Claude Desktop) at it:\n\
         \n  \
         export ANTHROPIC_BASE_URL={origin}\n  \
         export ANTHROPIC_AUTH_TOKEN={secret}\n\
         \n\
         The proxy swaps that loopback token for a short-lived gateway JWT,\n\
         injects the canonical identity headers, and refreshes in the\n\
         background. Press Ctrl-C to stop.\n",
        bin = crate::brand::brand().binary_name,
    ));

    let (tx, rx) = channel::<()>();
    match ctrlc::set_handler(move || {
        _ = tx.send(());
    }) {
        Ok(()) => {
            _ = rx.recv();
            crate::proxy::portfile::clear();
            output::print_str(&format!(
                "\n{} proxy stopped.\n",
                crate::brand::brand().binary_name
            ));
            ExitCode::SUCCESS
        },
        Err(e) => {
            diag(&format!(
                "proxy: Ctrl-C handler unavailable ({e}); running until killed"
            ));
            loop {
                std::thread::park();
            }
        },
    }
}
