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
    let egress_allowed_hosts = parse_opt_flag(args, "--egress-allowed-hosts")
        .as_deref()
        .and_then(install::parse_egress_allowed_hosts);
    match install::install(
        &install::InstallOptions {
            print_mdm,
            emit_schedule_template: emit_sched,
            gateway_url: gateway,
            pubkey,
            apply,
            apply_mobileconfig,
            apply_schedule,
            egress_allowed_hosts,
        },
        ctx,
    ) {
        Ok(summary) => {
            stdio::print_str(&install::render_install_summary(&summary));
            // Why: the stale-secret remediation has always named this command
            // ("or `<bin> install --apply`"), but nothing here touched a host
            // profile — `install_profile` had one caller, the GUI button. The
            // advice is now true: an --apply repairs the profiles that have
            // gone stale, and leaves hosts that were never set up alone.
            if apply {
                let overrides = crate::integration::reapply::ModelProtocolOverrides::new();
                let reports = ctx.block_on(crate::integration::reapply::reapply_stale_profiles(
                    ctx, &overrides,
                ));
                stdio::print_str(&crate::integration::reapply::render(&reports));
            }
            ExitCode::SUCCESS
        },
        Err(err) => {
            diag(&err.to_string());
            install::InstallError::EXIT_CODE
        },
    }
}
