//! `login` command: stores a PAT obtained by single sign-on, pasted directly,
//! or redeemed from an admin-issued one-shot exchange code.
//!
//! Device-link *authentication* (as opposed to this one-time bootstrap) is
//! interactive per request, which is why it cannot back the proxy; a device
//! certificate (`admin bridge enroll-cert`) is the only credential that renews
//! unattended.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::process::ExitCode;

use systemprompt_identifiers::{SessionId, ValidatedUrl};

use crate::auth::loopback::{LOOPBACK_PORT, LoopbackServer};
use crate::auth::providers::session::{capture_on, device_link_url};
use crate::auth::setup;
use crate::auth::types::SessionPatRequest;
use crate::cli::args::{has_flag, parse_opt_flag};
use crate::cli::output;
use crate::gateway::GatewayClient;
use crate::obs::output::diag;

pub fn cmd_login(args: &[String]) -> ExitCode {
    let gateway = parse_opt_flag(args, "--gateway");
    let device_name = parse_opt_flag(args, "--device-name");
    let pasted_pat = args.get(2).filter(|t| !t.is_empty() && !t.starts_with('-'));

    let code = if let Some(code) = parse_opt_flag(args, "--code") {
        Some(code)
    } else if pasted_pat.is_none() {
        match sso_code(gateway.as_deref(), has_flag(args, "--no-browser")) {
            Ok(c) => Some(c),
            Err(e) => {
                diag(&format!("login: single sign-on failed: {e}"));
                return ExitCode::from(1);
            },
        }
    } else {
        None
    };

    let token = if let Some(code) = code {
        match redeem_code(&code, gateway.as_deref(), device_name) {
            Ok(pat) => pat,
            Err(e) => {
                diag(&format!("login: could not redeem the exchange code: {e}"));
                return ExitCode::from(1);
            },
        }
    } else {
        let Some(t) = pasted_pat else {
            diag(&usage());
            return ExitCode::from(64);
        };
        crate::ids::PatToken::new(t.clone())
    };

    match setup::login(token.as_str(), gateway.as_deref()) {
        Ok(paths) => {
            let bin = crate::brand::brand().binary_name;
            output::print_line(&format!("Stored PAT for {bin} helper."));
            output::print_line(&format!("  config: {}", paths.config_file.display()));
            output::print_line(&format!("  secret: {} (0600)", paths.pat_file.display()));
            output::print_line(&format!("Next: run `{bin}` to fetch a JWT."));
            ExitCode::SUCCESS
        },
        Err(e) => {
            diag(&format!("login failed: {e}"));
            ExitCode::from(1)
        },
    }
}

fn usage() -> String {
    format!(
        "usage: {bin} login [--gateway <url>] [--no-browser] [--device-name <name>]\n   \
         or: {bin} login <sp-live-...> [--gateway <url>]\n   \
         or: {bin} login --code <exchange-code> [--gateway <url>] [--device-name <name>]\n\
         \n\
         With no token or code, signs in through the gateway's device-link page:\n\
         you authenticate with your organisation's identity provider and the\n\
         resulting token is bound to that identity. Use --no-browser on a machine\n\
         with no browser (SSH, headless) to open the URL elsewhere and paste the\n\
         redirect back.\n\
         \n\
         --code takes a one-shot code an administrator issued with:\n  \
         systemprompt admin bridge issue-code --user-id <uuid>\n\
         That path asserts your identity rather than proving it, so prefer SSO\n\
         wherever a browser is reachable.",
        bin = crate::brand::brand().binary_name
    )
}

fn sso_code(gateway: Option<&str>, no_browser: bool) -> Result<String, String> {
    let base_url = resolve_gateway(gateway)?;

    if !no_browser {
        return crate::proxy::block_on(async move {
            let server = LoopbackServer::bind()
                .await
                .map_err(|e| format!("could not bind the loopback callback listener: {e}"))?;
            capture_on(server, &base_url)
                .await
                .map_err(|e| e.to_string())
        })
        .map_err(|e| format!("runtime init: {e}"))?;
    }

    let callback = format!("http://127.0.0.1:{LOOPBACK_PORT}/callback");
    let url = device_link_url(base_url.as_str(), &callback);
    output::print_line("Open this URL on a machine with a browser and sign in:");
    output::print_line("");
    output::print_line(&format!("    {url}"));
    output::print_line("");
    output::print_line(
        "After approving, the browser will fail to reach 127.0.0.1 — that is expected.",
    );
    output::print_line("Copy the full address it landed on and paste it here.");
    output::print_line("");

    let mut line = String::new();
    std::io::stdin()
        .read_line(&mut line)
        .map_err(|e| format!("could not read the pasted URL: {e}"))?;
    extract_code(line.trim())
}

fn extract_code(pasted: &str) -> Result<String, String> {
    if pasted.is_empty() {
        return Err("nothing pasted".into());
    }
    let Some((_, query)) = pasted.split_once('?') else {
        return Ok(pasted.to_owned());
    };
    for pair in query.split('&') {
        if let Some(value) = pair.strip_prefix("code=") {
            let code = value.split('#').next().unwrap_or(value);
            if !code.is_empty() {
                return Ok(code.to_owned());
            }
        }
        if let Some(reason) = pair.strip_prefix("error=") {
            return Err(format!("the sign-in was not approved ({reason})"));
        }
    }
    Err("that URL carries no `code` parameter — paste the address the browser finished on".into())
}

fn resolve_gateway(gateway: Option<&str>) -> Result<ValidatedUrl, String> {
    gateway.map_or_else(
        || Ok(crate::config::gateway_url_or_default(&crate::config::load())),
        |raw| ValidatedUrl::try_new(raw.trim()).map_err(|e| format!("--gateway: {e}")),
    )
}

fn redeem_code(
    code: &str,
    gateway: Option<&str>,
    device_name: Option<String>,
) -> Result<crate::ids::PatToken, String> {
    let base_url = resolve_gateway(gateway)?;
    let req = SessionPatRequest {
        code: code.trim().to_owned(),
        device_name: device_name.or_else(default_device_name),
    };
    let client = GatewayClient::new(base_url);
    crate::proxy::block_on(async move {
        client
            .session_pat_exchange(&req, &SessionId::generate())
            .await
    })
    .map_err(|e| format!("runtime init: {e}"))?
    .map_err(|e| e.to_string())
}

fn default_device_name() -> Option<String> {
    std::env::var("HOSTNAME")
        .ok()
        .map(|h| h.trim().to_owned())
        .filter(|h| !h.is_empty())
        .or_else(|| {
            std::fs::read_to_string("/etc/hostname")
                .ok()
                .map(|h| h.trim().to_owned())
                .filter(|h| !h.is_empty())
        })
}
