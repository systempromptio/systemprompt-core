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

use std::io::IsTerminal;
use std::process::ExitCode;

use systemprompt_identifiers::{SessionId, ValidatedUrl};

use crate::auth::loopback::LoopbackServer;
use crate::auth::providers::session::{capture_on, device_link_url};
use crate::auth::setup;
use crate::cli::args::{has_flag, parse_opt_flag};
use crate::context::BridgeContext;
use crate::gateway::types::SessionPatRequest;
use crate::stdio;
use crate::stdio::diag;

pub fn cmd_login(ctx: &BridgeContext, args: &[String]) -> ExitCode {
    let gateway = parse_opt_flag(args, "--gateway");
    let device_name = parse_opt_flag(args, "--device-name");
    let pasted_pat = args.get(2).filter(|t| !t.is_empty() && !t.starts_with('-'));

    let code = if let Some(code) = parse_opt_flag(args, "--code") {
        code
    } else if let Some(t) = pasted_pat {
        let token = crate::ids::PatToken::new(t.clone());
        return finish_login(ctx, token, gateway, args);
    } else {
        match sso_code(ctx, gateway.as_deref(), has_flag(args, "--no-browser")) {
            Ok(c) => c,
            Err(e) => {
                diag(&format!("login: single sign-on failed: {e}"));
                return ExitCode::from(1);
            },
        }
    };

    let token = match redeem_code(ctx, &code, gateway.as_deref(), device_name) {
        Ok(pat) => pat,
        Err(e) => {
            diag(&format!("login: could not redeem the exchange code: {e}"));
            return ExitCode::from(1);
        },
    };

    finish_login(ctx, token, gateway, args)
}

fn finish_login(
    ctx: &BridgeContext,
    token: crate::ids::PatToken,
    gateway: Option<String>,
    args: &[String],
) -> ExitCode {
    match setup::login(token.as_str(), gateway.as_deref()) {
        Ok(paths) => {
            let bin = crate::brand::brand().binary_name;
            stdio::print_line(&format!("Stored PAT for {bin} helper."));
            stdio::print_line(&format!("  config: {}", paths.config_file.display()));
            stdio::print_line(&format!("  secret: {} (0600)", paths.pat_file.display()));
            stdio::print_line(&format!("Next: run `{bin}` to fetch a JWT."));
            reapply_after_login(ctx, has_flag(args, "--no-reapply"));
            ExitCode::SUCCESS
        },
        Err(e) => {
            diag(&format!("login failed: {e}"));
            ExitCode::from(1)
        },
    }
}

fn sso_code(
    ctx: &BridgeContext,
    gateway: Option<&str>,
    no_browser: bool,
) -> Result<String, String> {
    // Why: both SSO paths need a person — one waits on a browser callback, the
    // other on a pasted code. Detached from a terminal neither can ever
    // complete, so they would block until the caller gives up rather than
    // naming the one credential that works unattended.
    if !std::io::stdin().is_terminal() {
        return Err(format!(
            "signing in interactively needs a terminal. Unattended, redeem an \
             administrator-issued code with `{bin} login --code <exchange-code>`, or \
             enrol a device certificate, which is the only credential that renews \
             without a person present",
            bin = crate::brand::brand().binary_name
        ));
    }

    let base_url = resolve_gateway(gateway)?;

    if !no_browser {
        return ctx.block_on(async move {
            let server = LoopbackServer::bind()
                .await
                .map_err(|e| format!("could not bind the loopback callback listener: {e}"))?;
            capture_on(server, &base_url)
                .await
                .map_err(|e| e.to_string())
        });
    }

    let url = device_link_url(base_url.as_str(), None);
    stdio::print_line("Open this URL on a machine with a browser and sign in:");
    stdio::print_line("");
    stdio::print_line(&format!("    {url}"));
    stdio::print_line("");
    stdio::print_line("After approving, the page shows a one-time code. Paste it here:");
    stdio::print_line("");

    let mut line = String::new();
    std::io::stdin()
        .read_line(&mut line)
        .map_err(|e| format!("could not read the pasted code: {e}"))?;
    extract_code(line.trim())
}

fn extract_code(pasted: &str) -> Result<String, String> {
    let pasted = strip_terminal_noise(pasted);
    let pasted = pasted.trim();
    if pasted.is_empty() {
        return Err("nothing pasted".into());
    }

    // Why: first, because the displayed command carries a `--gateway` URL that
    // a query-string parse would otherwise wander into.
    if let Some(code) = code_after_flag(pasted) {
        return Ok(code);
    }

    let Some((_, query)) = pasted.split_once('?') else {
        if pasted.split_whitespace().count() > 1 {
            return Err(
                "that looks like a command but carries no `--code` — paste just the code, or the \
                 whole command the page displayed"
                    .into(),
            );
        }
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
    Err("that URL carries no `code` parameter — paste the code the page displayed".into())
}

// Why: terminals with bracketed paste enabled wrap the paste in `ESC[200~` /
// `ESC[201~`. Readline strips those at a shell prompt but nothing strips them
// from a raw stdin read, so without this the gateway rejects a code the user
// can see is correct. A non-CSI escape is two characters, hence dropping one.
fn strip_terminal_noise(pasted: &str) -> String {
    let mut out = String::with_capacity(pasted.len());
    let mut chars = pasted.chars();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            if chars.next() == Some('[') {
                for tail in chars.by_ref() {
                    if ('\u{40}'..='\u{7e}').contains(&tail) {
                        break;
                    }
                }
            }
            continue;
        }
        if !c.is_control() || c.is_whitespace() {
            out.push(c);
        }
    }
    out
}

fn code_after_flag(pasted: &str) -> Option<String> {
    let mut tokens = pasted.split_whitespace();
    while let Some(token) = tokens.next() {
        if let Some(code) = token.strip_prefix("--code=") {
            return (!code.is_empty()).then(|| code.to_owned());
        }
        if token == "--code" {
            return tokens.next().map(str::to_owned).filter(|c| !c.is_empty());
        }
    }
    None
}

fn resolve_gateway(gateway: Option<&str>) -> Result<ValidatedUrl, String> {
    gateway.map_or_else(
        || Ok(crate::config::gateway_url_or_default(&crate::config::load())),
        |raw| ValidatedUrl::try_new(raw.trim()).map_err(|e| format!("--gateway: {e}")),
    )
}

fn redeem_code(
    ctx: &BridgeContext,
    code: &str,
    gateway: Option<&str>,
    device_name: Option<String>,
) -> Result<crate::ids::PatToken, String> {
    let base_url = resolve_gateway(gateway)?;
    let req = SessionPatRequest {
        code: code.trim().to_owned(),
        device_name: device_name.or_else(default_device_name),
    };
    let client = ctx.gateway_client(base_url);
    ctx.block_on(async move {
        client
            .session_pat_exchange(&req, &SessionId::generate())
            .await
    })
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

// Why: signing in re-mints the credential and can move the gateway, but
// installed host profiles keep the loopback secret they were written with —
// see `integration::reapply`. The TTY gate is the load-bearing part here: an
// interactive sign-in can answer the administrator prompt a managed profile
// may raise, a scripted one cannot, and must not stall on a dialog nobody is
// there to see.
fn reapply_after_login(ctx: &BridgeContext, opted_out: bool) {
    use std::io::IsTerminal as _;

    if opted_out {
        return;
    }
    if !std::io::stdin().is_terminal() {
        stdio::print_line(
            "Not a terminal \u{2014} skipping host-profile repair. Run `install --apply` to \
             refresh any profile whose loopback secret has moved on.",
        );
        return;
    }
    let overrides = crate::integration::reapply::ModelProtocolOverrides::new();
    let reports = ctx.block_on(crate::integration::reapply::reapply_stale_profiles(
        ctx, &overrides,
    ));
    if !reports.is_empty() {
        stdio::print_str(&crate::integration::reapply::render(&reports));
    }
}

#[path = "login_test_api.rs"]
pub mod test_api;
