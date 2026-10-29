//! `gateway list|add|use|remove`, `disable` and `enable`: the gateway profiles
//! on this install and the user's opt-in disable.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::process::ExitCode;

use crate::config::gateways::{self, GatewayProfileId};
use crate::context::BridgeContext;
use crate::integration::lifecycle::{self, Released};
use crate::stdio::{self, diag};

const USAGE: &str =
    "usage: gateway list | gateway add <url> | gateway use <id> | gateway remove <id>";

pub fn cmd_gateway(ctx: &BridgeContext, args: &[String]) -> ExitCode {
    let positionals: Vec<&str> = args
        .iter()
        .skip(2)
        .map(String::as_str)
        .filter(|a| !a.starts_with('-'))
        .collect();
    match positionals.as_slice() {
        ["list"] | [] => list(),
        ["add", url] => add(ctx, url),
        ["use", id] => with_id(id, |id| use_profile(ctx, &id)),
        ["remove", id] => with_id(id, |id| remove(ctx, &id)),
        _ => {
            diag(USAGE);
            ExitCode::from(64)
        },
    }
}

fn with_id(raw: &str, run: impl FnOnce(GatewayProfileId) -> ExitCode) -> ExitCode {
    match GatewayProfileId::try_new(raw) {
        Ok(id) => run(id),
        Err(e) => {
            diag(&format!("gateway: {e}"));
            ExitCode::from(64)
        },
    }
}

fn list() -> ExitCode {
    match gateways::list() {
        Ok(profiles) => {
            for p in profiles {
                let marker = if p.active { "*" } else { " " };
                let url = p.gateway_url.as_deref().unwrap_or("(default gateway)");
                let name = p
                    .display_name
                    .map_or_else(String::new, |n| format!("  {n}"));
                let state = if p.disabled { "  [disabled]" } else { "" };
                stdio::print_line(&format!("{marker} {}  {url}{name}{state}", p.id));
            }
            ExitCode::SUCCESS
        },
        Err(e) => {
            diag(&format!("gateway list: {e}"));
            ExitCode::from(1)
        },
    }
}

fn add(ctx: &BridgeContext, url: &str) -> ExitCode {
    let id = match gateways::add(url) {
        Ok(id) => id,
        Err(e) => {
            diag(&format!("gateway add: {e}"));
            return ExitCode::from(1);
        },
    };
    stdio::print_line(&format!("Added gateway profile `{id}` for {url}."));
    let http = ctx.http.clone();
    match gateways::profile(&id) {
        Ok(profile) => {
            match ctx.block_on(crate::gateway::discovery::refresh_profile(&http, &profile)) {
                Ok(crate::gateway::discovery::DiscoveryOutcome::Fetched(doc)) => {
                    if let Some(name) = doc.branding.display_name {
                        stdio::print_line(&format!("  instance: {name}"));
                    }
                },
                Ok(crate::gateway::discovery::DiscoveryOutcome::NotServed) => {},
                Err(e) => diag(&format!("gateway add: discovery unavailable ({e})")),
            }
        },
        Err(e) => diag(&format!("gateway add: {e}")),
    }
    report_machine_profile(ctx);
    stdio::print_line(&format!(
        "Run `{} gateway use {id}` to switch to it, then sign in.",
        crate::brand::brand().binary_name
    ));
    ExitCode::SUCCESS
}

fn use_profile(ctx: &BridgeContext, id: &GatewayProfileId) -> ExitCode {
    let report = match lifecycle::switch(ctx, id) {
        Ok(report) => report,
        Err(e) => {
            diag(&format!("gateway use: {e}"));
            return ExitCode::from(1);
        },
    };
    print_released(&report.released);
    stdio::print_line(&format!("Active gateway is now `{}`.", report.profile.id));
    let http = ctx.http.clone();
    if let Err(e) = ctx.block_on(crate::gateway::discovery::refresh_active(&http)) {
        diag(&format!(
            "gateway use: discovery unavailable ({e}); using built-in routes"
        ));
    }
    follow_up_sync(ctx)
}

fn remove(ctx: &BridgeContext, id: &GatewayProfileId) -> ExitCode {
    match gateways::remove(id) {
        Ok(()) => {
            stdio::print_line(&format!(
                "Removed gateway profile `{id}` and its stored PAT."
            ));
            report_machine_profile(ctx);
            ExitCode::SUCCESS
        },
        Err(e) => {
            diag(&format!("gateway remove: {e}"));
            ExitCode::from(1)
        },
    }
}

pub fn cmd_disable(ctx: &BridgeContext) -> ExitCode {
    match lifecycle::disable(ctx) {
        Ok(report) => {
            if report.already_disabled {
                stdio::print_line("The bridge is already disabled on this device.");
            } else {
                print_released(&report.released);
                stdio::print_line(
                    "Disabled. Hosts use their own sign-in until you run `enable`; the gateway \
                     still sees this device as disabled.",
                );
            }
            for host in &report.still_managed {
                stdio::print_line(&format!(
                    "  still managed: {host} — set by an administrator, not changed by disable"
                ));
            }
            ExitCode::SUCCESS
        },
        Err(e) => {
            diag(&format!("disable: {e}"));
            ExitCode::from(1)
        },
    }
}

pub fn cmd_enable(ctx: &BridgeContext) -> ExitCode {
    match lifecycle::enable(ctx) {
        Ok(false) => {
            stdio::print_line("The bridge is not disabled.");
            ExitCode::SUCCESS
        },
        Ok(true) => {
            stdio::print_line("Enabled. Syncing from the gateway…");
            let code = follow_up_sync(ctx);
            if code == ExitCode::SUCCESS {
                re_enrol(ctx);
            }
            code
        },
        Err(e) => {
            diag(&format!("enable: {e}"));
            ExitCode::from(1)
        },
    }
}

fn print_released(released: &Released) {
    for name in &released.foreign_plugins {
        stdio::print_line(&format!(
            "  left in place (not placed by the bridge): {name}"
        ));
    }
    for warning in &released.warnings {
        stdio::print_line(&format!("  warning: {warning}"));
    }
}

fn follow_up_sync(ctx: &BridgeContext) -> ExitCode {
    let options = crate::sync::SyncOptions {
        allow_unsigned: false,
        force_replay: false,
        allow_tofu: false,
        freshness: crate::gateway::Freshness::Fresh,
        cancel: tokio_util::sync::CancellationToken::new(),
    };
    match ctx.block_on(crate::sync::run_once(ctx, &options)) {
        Ok(summary) => {
            stdio::print_line(&summary.one_line());
            let overrides = crate::integration::reapply::ModelProtocolOverrides::new();
            let reports = ctx.block_on(crate::integration::reapply::reapply_stale_profiles(
                ctx,
                &overrides,
                crate::integration::reapply::Attendance::Unattended,
            ));
            if !reports.is_empty() {
                stdio::print_str(&crate::integration::reapply::render(&reports));
            }
            ExitCode::SUCCESS
        },
        Err(err) => {
            diag(&format!("sync: {err}"));
            err.exit_code()
        },
    }
}

fn re_enrol(ctx: &BridgeContext) {
    match ctx.block_on(lifecycle::re_enrol_delivered_hosts(ctx)) {
        Ok(reports) if reports.is_empty() => {},
        Ok(reports) => stdio::print_str(&crate::integration::enrol::render(&reports)),
        Err(e) => diag(&format!("enable: host profiles not restored: {e}")),
    }
}

fn report_machine_profile(ctx: &BridgeContext) {
    use crate::integration::lifecycle::MachineProfileRefresh;
    match ctx.block_on(lifecycle::refresh_machine_profile(ctx)) {
        MachineProfileRefresh::Current => {},
        MachineProfileRefresh::Rewritten => {
            stdio::print_line("Claude Desktop's policy now covers every gateway profile.");
        },
        MachineProfileRefresh::AwaitingApproval => stdio::print_line(
            "Claude Desktop needs one administrator approval to cover every gateway profile; \
             run `install --apply` or approve it in the app.",
        ),
        MachineProfileRefresh::ManagedElsewhere { missing } if missing.is_empty() => {},
        MachineProfileRefresh::ManagedElsewhere { missing } => stdio::print_line(&format!(
            "Claude Desktop's policy is set by device management and does not pin: {}",
            missing
                .iter()
                .map(GatewayProfileId::as_str)
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}
