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

struct InstallArgs {
    print_mdm: Option<Os>,
    emit_sched: Option<Os>,
    gateway: Option<ValidatedUrl>,
    pubkey: Option<PinnedPubKey>,
    apply: bool,
    apply_mobileconfig: bool,
    apply_schedule: bool,
    egress_allowed_hosts: Option<Vec<String>>,
    host_selection: Option<Selection>,
}

fn parse_install_args(args: &[String]) -> Result<InstallArgs, ExitCode> {
    let print_mdm = os_flag(args, "--print-mdm")?;
    let emit_sched = os_flag(args, "--emit-schedule-template")?;
    let gateway = match parse_opt_flag(args, "--gateway") {
        Some(raw) => match ValidatedUrl::try_new(raw.trim()) {
            Ok(url) => Some(url),
            Err(e) => {
                diag(&format!("--gateway: invalid URL: {e}"));
                return Err(ExitCode::from(64));
            },
        },
        None => None,
    };
    let egress_allowed_hosts = match parse_opt_flag(args, "--egress-allowed-hosts") {
        None => None,
        Some(raw) => match install::parse_egress_allowed_hosts(&raw) {
            Ok(hosts) => Some(hosts),
            Err(e) => {
                diag(&format!("--egress-allowed-hosts: {e}"));
                return Err(ExitCode::from(64));
            },
        },
    };
    let host_selection = match parse_host_selection(args) {
        Ok(selection) => selection,
        Err(e) => {
            diag(&e.to_string());
            return Err(ExitCode::from(64));
        },
    };
    Ok(InstallArgs {
        print_mdm,
        emit_sched,
        gateway,
        pubkey: parse_opt_flag(args, "--pubkey").map(PinnedPubKey::new),
        apply: has_flag(args, "--apply"),
        apply_mobileconfig: has_flag(args, "--apply-mobileconfig"),
        apply_schedule: has_flag(args, "--apply-schedule"),
        egress_allowed_hosts,
        host_selection,
    })
}

fn refresh_model_catalog(
    ctx: &BridgeContext,
    gateway: Option<&ValidatedUrl>,
) -> Result<(), ExitCode> {
    let cfg = match crate::config::load() {
        Ok(cfg) => cfg,
        Err(e) => {
            diag(&e.to_string());
            return Err(ExitCode::from(1));
        },
    };
    let gateway_url = gateway
        .cloned()
        .unwrap_or_else(|| crate::config::gateway_url_or_default(&cfg));
    let client = ctx.gateway_client(gateway_url);
    match ctx.block_on(crate::auth::profile::fetch_signed_in(&client)) {
        Ok(_) => Ok(()),
        Err(e) => {
            diag(&format!(
                "Cannot refresh the Desktop model catalog: {e}. Install was not applied."
            ));
            Err(ExitCode::from(1))
        },
    }
}

pub(super) fn cmd_install(ctx: &BridgeContext, args: &[String]) -> ExitCode {
    let parsed = match parse_install_args(args) {
        Ok(parsed) => parsed,
        Err(code) => return code,
    };
    let InstallArgs {
        print_mdm,
        emit_sched,
        gateway,
        pubkey,
        apply,
        apply_mobileconfig,
        apply_schedule,
        egress_allowed_hosts,
        host_selection,
    } = parsed;
    if (apply || apply_mobileconfig)
        && let Err(code) = refresh_model_catalog(ctx, gateway.as_ref())
    {
        return code;
    }
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
            if apply {
                let overrides = crate::integration::reapply::ModelProtocolOverrides::new();
                let reports = ctx.block_on(crate::integration::reapply::reapply_stale_profiles(
                    ctx,
                    &overrides,
                    crate::integration::reapply::Attendance::Attended,
                ));
                stdio::print_str(&crate::integration::reapply::render(&reports));
            }
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

#[derive(Debug, thiserror::Error)]
enum HostSelectionError {
    #[error("--hosts takes only 'all'; name individual hosts with --host <id>")]
    HostsNotAll,
    #[error("pass either --hosts all or --host <id>, not both")]
    Both,
    #[error(transparent)]
    Selection(#[from] enrol::SelectionError),
}

fn parse_host_selection(args: &[String]) -> Result<Option<Selection>, HostSelectionError> {
    let ids = parse_multi_flag(args, "--host");
    let all = parse_multi_flag(args, "--hosts");
    if !all.is_empty() && all.iter().any(|v| v != "all") {
        return Err(HostSelectionError::HostsNotAll);
    }
    match (ids.is_empty(), all.is_empty()) {
        (true, true) => Ok(None),
        (false, true) => Ok(Some(Selection::parse_ids(&ids)?)),
        (true, false) => Ok(Some(Selection::All)),
        (false, false) => Err(HostSelectionError::Both),
    }
}

fn enrol_selected(ctx: &BridgeContext, selection: &Selection) -> ExitCode {
    let overrides = crate::integration::reapply::ModelProtocolOverrides::new();
    let enabled = match crate::sync::last_synced_enabled_hosts() {
        Ok(enabled) => enabled,
        Err(e) => {
            diag(&format!("last-sync sentinel: {e}"));
            return ExitCode::from(1);
        },
    };
    // Why: a host profile that names the managed servers (Claude Desktop) is
    // written from the registry; a fresh fetch keeps it from repeating the
    // server set of a sync that ran before the user linked a connector.
    if let Err(e) = ctx.block_on(crate::sync::refresh_registry(ctx)) {
        diag(&format!(
            "managed MCP servers not refreshed from the gateway ({e}); the profiles are written \
             from the last sync's server list"
        ));
    }
    match ctx.block_on(enrol::enrol_hosts(ctx, selection, &overrides, enabled)) {
        Ok(reports) => {
            stdio::print_str(&enrol::render(&reports));
            if reports.iter().any(enrol::Report::is_failure) {
                ExitCode::from(1)
            } else {
                ExitCode::SUCCESS
            }
        },
        Err(e) => {
            diag(&e.to_string());
            ExitCode::from(64)
        },
    }
}

fn os_flag(args: &[String], flag: &str) -> Result<Option<Os>, ExitCode> {
    let Some(raw) = parse_opt_flag(args, flag) else {
        return Ok(None);
    };
    Os::parse(&raw).map(Some).ok_or_else(|| {
        diag(&format!(
            "{flag}: expected macos, windows or linux, got {raw:?}"
        ));
        ExitCode::from(64)
    })
}
