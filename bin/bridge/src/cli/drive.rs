//! `drive` command: Drive folder status, a one-shot sync and the open
//! conflicts.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::process::ExitCode;

use crate::context::BridgeContext;
use crate::drive::{Cycle, DriveClient, DriveError, DriveRoot, DriveStatus};
use crate::stdio::{self, diag};

pub fn cmd_drive(ctx: &BridgeContext, args: &[String]) -> ExitCode {
    let Some(root) = DriveRoot::resolve() else {
        diag("drive: this build has no workspace folder");
        return ExitCode::from(69);
    };
    match args.get(2).map(String::as_str) {
        None | Some("status") => print_status(&root),
        Some("conflicts") => print_conflicts(&root),
        Some("sync") => sync(ctx, &root),
        Some(other) => {
            diag(&format!(
                "drive: unknown subcommand {other:?}; use status, sync or conflicts"
            ));
            ExitCode::from(64)
        },
    }
}

fn load_status(root: &DriveRoot) -> Result<DriveStatus, ExitCode> {
    match DriveStatus::from_disk(root) {
        Ok(status) => Ok(status),
        Err(error) => {
            diag(&format!("drive: {error}"));
            Err(ExitCode::FAILURE)
        },
    }
}

fn print_status(root: &DriveRoot) -> ExitCode {
    let status = match load_status(root) {
        Ok(status) => status,
        Err(code) => return code,
    };
    stdio::print_line(&format!("drive folder: {}", root.dir().display()));
    if !status.enabled {
        stdio::print_line("  state: not set up (no Drive folder yet)");
        return ExitCode::SUCCESS;
    }
    stdio::print_line(&format!("  pending uploads: {}", status.pending_uploads));
    stdio::print_line(&format!("  conflicts: {}", status.conflicts.len()));
    ExitCode::SUCCESS
}

fn print_conflicts(root: &DriveRoot) -> ExitCode {
    let status = match load_status(root) {
        Ok(status) => status,
        Err(code) => return code,
    };
    if status.conflicts.is_empty() {
        stdio::print_line("no drive conflicts");
    }
    for notice in &status.conflicts {
        stdio::print_line(&format!(
            "{}  kept local edit as {}  (head {}, {})",
            notice.path, notice.sibling, notice.head_revision_id, notice.detected_at
        ));
    }
    ExitCode::SUCCESS
}

fn sync(ctx: &BridgeContext, root: &DriveRoot) -> ExitCode {
    match ctx.block_on(sync_once(ctx, root)) {
        Ok(report) => {
            stdio::print_line(&format!(
                "drive synced: {} pulled, {} committed, {} merged, {} conflicts, {} pending",
                report.pull.applied,
                report.upload.committed,
                report.upload.merged,
                report.pull.conflicts.len() + report.upload.conflicts.len(),
                report.pending_uploads,
            ));
            ExitCode::SUCCESS
        },
        Err(error) => {
            tracing::error!(%error, "drive sync failed");
            diag(&format!("drive sync failed: {error}"));
            ExitCode::FAILURE
        },
    }
}

async fn sync_once(
    ctx: &BridgeContext,
    root: &DriveRoot,
) -> Result<crate::drive::run::CycleReport, DriveError> {
    let cfg = crate::config::load().map_err(DriveError::Config)?;
    let gateway = crate::config::gateway_url_or_default(&cfg);
    let session_id = crate::auth::session_identity::stable_session_id(&cfg);
    let bearer = crate::auth::obtain_live_token(&cfg, &session_id, &ctx.http)
        .await
        .map_err(DriveError::Credential)?
        .token;
    let device_id = systemprompt_identifiers::DriveDeviceId::try_new(ctx.install_id().as_str())
        .map_err(DriveError::DeviceId)?;
    let host_name = crate::sysproc::host_name().unwrap_or_else(|| "this device".to_owned());
    let client = DriveClient::new(gateway, ctx.http.clone());
    let cycle = Cycle {
        client: &client,
        bearer: &bearer,
        root,
        device_id: &device_id,
        host_name: &host_name,
    };
    if !crate::drive::run::ensure_enabled(cycle, &ctx.drive).await? {
        return Ok(crate::drive::run::CycleReport::default());
    }
    crate::drive::run::sync_once(cycle, &ctx.drive).await
}
