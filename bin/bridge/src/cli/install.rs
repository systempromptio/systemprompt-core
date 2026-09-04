//! `install` command: installs the bridge binary and scheduled sync task, and
//! optionally enrols named host applications (`--host <id>`, `--hosts all`).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::process::ExitCode;

use systemprompt_identifiers::ValidatedUrl;

use crate::cli::args::{has_flag, parse_multi_flag, parse_opt_flag};
use crate::context::BridgeContext;
use crate::ids::PinnedPubKey;
use crate::integration::enrol::{self, Selection};
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
    let host_selection = match parse_host_selection(args) {
        Ok(sel) => sel,
        Err(msg) => {
            diag(&msg);
            return ExitCode::from(64);
        },
    };
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
            // Why: repairs the profiles that have gone stale and leaves
            // hosts that were never set up alone, so the stale-secret
            // remediation can keep naming this command.
            if apply {
                let overrides = crate::integration::reapply::ModelProtocolOverrides::new();
                let reports = ctx.block_on(crate::integration::reapply::reapply_stale_profiles(
                    ctx, &overrides,
                ));
                stdio::print_str(&crate::integration::reapply::render(&reports));
            }
            // Why: enrolment is deliberately independent of --apply. --apply
            // lands MDM policy and the scheduled task and only *repairs*
            // profiles that already exist; --host is how a client that was
            // never set up gets one, which is the whole Linux install path.
            host_selection.map_or(ExitCode::SUCCESS, |selection| {
                enrol_selected(ctx, &selection)
            })
        },
        Err(err) => {
            diag(&err.to_string());
            install::InstallError::EXIT_CODE
        },
    }
}

// Why: `--hosts all` and a repeated `--host` are the same request expressed
// two ways; naming both on one line is a contradiction rather than a union, so
// it is refused instead of guessed at.
fn parse_host_selection(args: &[String]) -> Result<Option<Selection>, String> {
    let ids = parse_multi_flag(args, "--host");
    let all = parse_multi_flag(args, "--hosts");
    if !all.is_empty() && all.iter().any(|v| v != "all") {
        return Err("--hosts takes only 'all'; name individual hosts with --host <id>".to_owned());
    }
    match (ids.is_empty(), all.is_empty()) {
        (true, true) => Ok(None),
        (false, true) => Ok(Some(Selection::Ids(ids))),
        (true, false) => Ok(Some(Selection::All)),
        (false, false) => Err("pass either --hosts all or --host <id>, not both".to_owned()),
    }
}

fn enrol_selected(ctx: &BridgeContext, selection: &Selection) -> ExitCode {
    let overrides = crate::integration::reapply::ModelProtocolOverrides::new();
    let enabled = crate::sync::last_synced_enabled_hosts();
    match ctx.block_on(enrol::enrol_hosts(ctx, selection, &overrides, enabled)) {
        Ok(reports) => {
            stdio::print_str(&enrol::render(&reports));
            if reports.iter().any(enrol::Report::is_failure) {
                ExitCode::from(1)
            } else {
                ExitCode::SUCCESS
            }
        },
        Err(msg) => {
            diag(&msg);
            ExitCode::from(64)
        },
    }
}
