//! `install` command: installs the bridge binary and scheduled sync task.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::process::ExitCode;

use systemprompt_identifiers::ValidatedUrl;

use crate::cli::args::{has_flag, parse_opt_flag};
use crate::context::BridgeContext;
use crate::ids::PinnedPubKey;
use crate::schedule::Os;
use crate::stdio::diag;
use crate::{install, stdio};

pub(super) fn cmd_install(ctx: &BridgeContext, args: &[String]) -> ExitCode {
    let print_mdm = parse_opt_flag(args, "--print-mdm")
        .as_deref()
        .and_then(Os::parse);
    let emit_sched = parse_opt_flag(args, "--emit-schedule-template")
        .as_deref()
        .and_then(Os::parse);
    let gateway = match parse_opt_flag(args, "--gateway") {
        Some(raw) => match ValidatedUrl::try_new(raw.trim()) {
            Ok(url) => Some(url),
            Err(e) => {
                diag(&format!("--gateway: invalid URL: {e}"));
                return ExitCode::from(64);
            },
        },
        None => None,
    };
    let pubkey = parse_opt_flag(args, "--pubkey").map(PinnedPubKey::new);
    let apply = has_flag(args, "--apply");
    let apply_mobileconfig = has_flag(args, "--apply-mobileconfig");
    let apply_schedule = has_flag(args, "--apply-schedule");
    // Why: recorded process-globally rather than threaded through InstallOptions
    // because the MDM payload renderers are reached from `--print-mdm` too, which
    // carries no options struct.
    _ = install::set_egress_allowed_hosts(
        parse_opt_flag(args, "--egress-allowed-hosts").as_deref(),
    );
    match install::install(
        &install::InstallOptions {
            print_mdm,
            emit_schedule_template: emit_sched,
            gateway_url: gateway,
            pubkey,
            apply,
            apply_mobileconfig,
            apply_schedule,
        },
        ctx.proxy.loopback(),
    ) {
        Ok(summary) => {
            stdio::print_str(&install::render_install_summary(&summary));
            ExitCode::SUCCESS
        },
        Err(err) => {
            diag(&err.to_string());
            install::InstallError::EXIT_CODE
        },
    }
}
