use std::process::ExitCode;
use std::sync::mpsc::channel;

use crate::cli::output;
use crate::obs::output::diag;

/// Start the local inference proxy headlessly and block until interrupted.
///
/// This is the Linux / headless equivalent of what the desktop GUI does on
/// macOS and Windows: it brings up the loopback proxy on
/// `127.0.0.1:{DEFAULT_PROXY_PORT}`, which swaps the printed loopback secret
/// for a short-lived gateway JWT and injects the canonical identity headers
/// (`x-session-id`, `x-user-id`, `x-trace-id`, …) on every forwarded request,
/// then refreshes the JWT in the background. Point any Anthropic-API client
/// (Claude Code, Claude Desktop) at the printed base URL + token.
///
/// Requires a configured credential from the auth provider chain (stored PAT,
/// inline PAT env var, or device certificate).
pub(super) fn cmd_proxy() -> ExitCode {
    if crate::proxy::start_default().is_none() {
        diag("proxy: failed to start (port in use, or no config dir) — see logs above");
        return ExitCode::from(1);
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
