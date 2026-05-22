//! `bridge oauth-client` subcommand.
//!
//! Operations on the bridge's per-tenant OAuth client used to mint plugin-
//! scoped hook tokens:
//!
//! - `status` — print the locally-stashed credentials (client_id, scopes, token
//!   endpoint). The plaintext secret is never echoed.
//! - `rotate` — call `/v1/auth/bridge/oauth-client` to obtain a fresh
//!   client_secret, persist it, and overwrite the cached value. Used when the
//!   local creds file is suspected lost or the tenant operator wants to
//!   invalidate any in-flight hook tokens.

use std::process::ExitCode;

use systemprompt_identifiers::SessionId;

use crate::auth::plugin_oauth;
use crate::cli::output;
use crate::gateway::GatewayClient;
use crate::obs::output::diag;
use crate::{auth, config};

pub(crate) fn cmd_oauth_client(args: &[String]) -> ExitCode {
    match args.get(2).map(String::as_str) {
        None | Some("status") => cmd_status(),
        Some("rotate") => cmd_rotate(),
        Some(other) => {
            diag(&format!("unknown oauth-client subcommand: {other}"));
            output::eprint_str("usage: systemprompt-bridge oauth-client [status | rotate]\n");
            ExitCode::from(64)
        },
    }
}

fn cmd_status() -> ExitCode {
    let path = match plugin_oauth::creds_path() {
        Some(p) => p,
        None => {
            diag("oauth-client status: cache directory unresolvable");
            return ExitCode::from(1);
        },
    };
    output::print_line(&format!("creds path: {}", path.display()));
    match plugin_oauth::load_creds() {
        Ok(Some(creds)) => {
            output::print_line(&format!("client_id: {}", creds.client_id));
            output::print_line(&format!("token endpoint: {}", creds.token_endpoint));
            output::print_line(&format!("scopes: {}", creds.scopes.join(" ")));
            ExitCode::SUCCESS
        },
        Ok(None) => {
            output::print_line(
                "status: not provisioned (run `bridge sync` once or `oauth-client rotate`)",
            );
            ExitCode::SUCCESS
        },
        Err(e) => {
            diag(&format!("oauth-client status: {e}"));
            ExitCode::from(1)
        },
    }
}

fn cmd_rotate() -> ExitCode {
    let cfg = config::load();
    let base_url = config::gateway_url_or_default(&cfg);
    let client = GatewayClient::new(base_url);

    let result = crate::proxy::block_on(async move {
        let bearer = auth::obtain_live_token(&cfg, &SessionId::generate())
            .await
            .ok_or("no credential source configured (run `bridge login` first)")?;
        let token = bearer.token.as_str().to_string();
        let creds = plugin_oauth::refresh_creds(&client, &token).await?;
        Ok::<_, Box<dyn std::error::Error>>(creds)
    });

    let outer = match result {
        Ok(o) => o,
        Err(e) => {
            diag(&format!("oauth-client rotate: runtime init failed: {e}"));
            return ExitCode::from(70);
        },
    };

    match outer {
        Ok(creds) => {
            output::print_line(&format!(
                "Rotated OAuth client secret for {}",
                creds.client_id
            ));
            output::print_line(&format!("scopes: {}", creds.scopes.join(" ")));
            output::print_line(&format!("token endpoint: {}", creds.token_endpoint));
            ExitCode::SUCCESS
        },
        Err(e) => {
            diag(&format!("oauth-client rotate: {e}"));
            ExitCode::from(1)
        },
    }
}
