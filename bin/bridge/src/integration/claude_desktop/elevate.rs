//! UAC elevation bridge for the Windows managed-policy write.
//! `SOFTWARE\Policies\Claude` is ACL-protected in both hives, so the write must
//! run in an elevated child (HKLM) that reports via a JSON result file.

use std::process::ExitCode;

use serde::{Deserialize, Serialize};

use crate::config::store::write_managed_claude_policy;
use crate::winproc::{ElevationOutcome, run_elevated};

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ElevatedResult {
    pub ok: bool,
    pub error: Option<String>,
}

pub(crate) fn perform_elevated_write(reg_path: &str, result_path: &str) -> ExitCode {
    let outcome = write_from_reg(reg_path);
    let result = match &outcome {
        Ok(()) => ElevatedResult {
            ok: true,
            error: None,
        },
        Err(e) => ElevatedResult {
            ok: false,
            error: Some(e.clone()),
        },
    };
    match serde_json::to_string(&result) {
        Ok(json) => {
            if let Err(e) = std::fs::write(result_path, &json) {
                tracing::warn!(error = %e, result_path, "failed to write elevated result file");
            }
        },
        Err(e) => tracing::warn!(error = %e, "failed to encode elevated result"),
    }
    if outcome.is_ok() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn write_from_reg(reg_path: &str) -> Result<(), String> {
    let body =
        std::fs::read_to_string(reg_path).map_err(|e| format!("read staged profile: {e}"))?;
    let entries = super::reg_profile::parse_reg_entries(&body);
    if entries.is_empty() {
        return Err("staged registry profile contained no policy values".into());
    }
    write_managed_claude_policy(true, &entries).map_err(|e| e.to_string())
}

pub(crate) fn elevate_and_install(reg_path: &str) -> std::io::Result<()> {
    let exe = std::env::current_exe()?;
    let result_path = format!("{reg_path}.result.json");
    tracing::info!(
        reg_path,
        "requesting elevation to write machine-wide Claude policy"
    );
    let outcome = run_elevated(
        &exe,
        &["__install-claude-policy", reg_path, result_path.as_str()],
    );
    match outcome {
        ElevationOutcome::Declined => Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "administrator approval was declined — the managed Claude policy was not written",
        )),
        ElevationOutcome::Failed(msg) => Err(std::io::Error::other(msg)),
        ElevationOutcome::Completed { exit_code } => finish(&result_path, exit_code),
    }
}

fn finish(result_path: &str, exit_code: u32) -> std::io::Result<()> {
    let detail = read_result(result_path);
    if exit_code == 0 && detail.as_ref().is_none_or(|r| r.ok) {
        return Ok(());
    }
    let message = detail
        .and_then(|r| r.error)
        .unwrap_or_else(|| format!("elevated policy write failed (exit code {exit_code})"));
    Err(std::io::Error::other(message))
}

fn read_result(result_path: &str) -> Option<ElevatedResult> {
    let body = std::fs::read_to_string(result_path).ok()?;
    serde_json::from_str(&body).ok()
}
