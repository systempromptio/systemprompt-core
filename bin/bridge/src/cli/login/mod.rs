//! `login` command: stores a PAT obtained by single sign-on, read from stdin
//! (`--stdin`), passed as an argument, or redeemed from an admin-issued
//! one-shot exchange code.
//!
//! Device-link *authentication* (as opposed to this one-time bootstrap) is
//! interactive per request, which is why it cannot back the proxy; the stored
//! PAT is the credential that renews unattended.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod pasted_code;

pub use pasted_code::{code_after_flag, extract_code, strip_terminal_noise};

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

    let code = if has_flag(args, "--stdin") {
        return match pat_from_stdin() {
            Ok(token) => finish_login(ctx, &token, gateway.as_deref(), args),
            Err(e) => {
                diag(&format!("login --stdin: {e}"));
                ExitCode::from(64)
            },
        };
    } else if let Some(code) = parse_opt_flag(args, "--code") {
        code
    } else if let Some(t) = pasted_pat {
        let token = crate::ids::PatToken::new(t.clone());
        return finish_login(ctx, &token, gateway.as_deref(), args);
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

    finish_login(ctx, &token, gateway.as_deref(), args)
}

fn pat_from_stdin() -> Result<crate::ids::PatToken, String> {
    let mut line = String::new();
    std::io::stdin()
        .read_line(&mut line)
        .map_err(|e| format!("could not read the PAT from stdin: {e}"))?;
    let token = line.trim();
    if token.is_empty() {
        return Err("stdin carried no PAT".to_owned());
    }
    Ok(crate::ids::PatToken::new(token))
}

fn finish_login(
    ctx: &BridgeContext,
    token: &crate::ids::PatToken,
    gateway: Option<&str>,
    args: &[String],
) -> ExitCode {
    match setup::login(token.as_str(), gateway) {
        Ok(paths) => {
            let bin = crate::brand::brand().binary_name;
            stdio::print_line(&format!("Stored PAT for {bin} helper."));
            stdio::print_line(&format!("  config: {}", paths.config_file.display()));
            stdio::print_line(&format!("  secret: {} (0600)", paths.pat_file.display()));
            stdio::print_line(&format!("Next: run `{bin}` to fetch a JWT."));
            enroll_device_after_login(ctx, gateway);
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
    if !std::io::stdin().is_terminal() {
        return Err(format!(
            "signing in interactively needs a terminal. Unattended, redeem an \
             administrator-issued code with `{bin} login --code <exchange-code>`, or \
             pipe a PAT into `{bin} login --stdin`",
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

pub fn resolve_gateway(gateway: Option<&str>) -> Result<ValidatedUrl, String> {
    gateway.map_or_else(
        || {
            crate::config::load()
                .map(|cfg| crate::config::gateway_url_or_default(&cfg))
                .map_err(|e| e.to_string())
        },
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

pub fn default_device_name() -> Option<String> {
    crate::sysproc::host_name()
}

// Why: attribution is best-effort — a login that stored a working PAT must
// succeed even when the gateway cannot enrol this device right now.
fn enroll_device_after_login(ctx: &BridgeContext, gateway: Option<&str>) {
    let result = (|| -> Result<systemprompt_identifiers::DeviceId, String> {
        let cfg = crate::config::load().map_err(|e| e.to_string())?;
        let base_url = resolve_gateway(gateway)?;
        let client = ctx.gateway_client(base_url);
        let http = ctx.http.clone();
        let install_id = ctx.install_id().clone();
        ctx.block_on(async move {
            let live = crate::auth::obtain_live_token(&cfg, &SessionId::generate(), &http)
                .await
                .map_err(|e| e.to_string())?;
            let whoami = client
                .fetch_whoami(&live.token)
                .await
                .map_err(|e| e.to_string())?;
            let user_id = whoami
                .user_id
                .ok_or_else(|| "whoami carried no user id".to_owned())?;
            let enrolment = crate::feedback::enrol::SelfEnrolment {
                install_id: install_id.as_str(),
                user_id: &user_id,
                label: default_device_name(),
                force_rotate: true,
            };
            crate::feedback::enrol::ensure_self_enrolled(&client, &live.token, &enrolment)
                .await
                .map(|enrollment| enrollment.device_id)
                .map_err(|e| e.to_string())
        })
    })();
    match result {
        Ok(device_id) => stdio::print_line(&format!(
            "Device {device_id} enrolled for installation feedback"
        )),
        Err(e) => diag(&format!("login: device enrolment skipped: {e}")),
    }
}

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
