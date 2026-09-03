//! UAC elevation bridge for the Windows machine-wide install steps.
//! `SOFTWARE\Policies\Claude` is ACL-protected in both hives and
//! `Program Files\Claude\org-plugins` is admin-write-only, so both are handled
//! by ONE elevated child driven by a staged JSON job file, reporting via a
//! JSON result file. A single UAC approval covers the policy write and the
//! org-plugins provisioning; afterwards unelevated `sync` can publish plugins.
//! The same child also lands admin-owned managed config files for other hosts
//! (`managed_files` / `remove_files`), so every machine-wide write shares one
//! tested elevation path.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use serde::{Deserialize, Serialize};

use crate::config::store::{
    clear_managed_claude_policy, write_bridge_policy, write_managed_claude_policy,
};
use crate::winproc::{ElevationOutcome, run_elevated};

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ElevatedJob {
    pub reg_path: Option<String>,
    pub org_plugins: Option<OrgPluginsJob>,
    #[serde(default)]
    pub clear_values: Vec<String>,
    #[serde(default)]
    pub bridge_values: Vec<(String, String)>,
    #[serde(default)]
    pub managed_files: Vec<ManagedFileJob>,
    #[serde(default)]
    pub remove_files: Vec<PathBuf>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ManagedFileJob {
    pub staged: PathBuf,
    pub dest: PathBuf,
}

// Why: `grant_user` is captured by the UNELEVATED parent — the elevated
// child may run as a different admin account, so it must never re-read
// `%USERNAME%` itself.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct OrgPluginsJob {
    pub path: PathBuf,
    pub grant_user: String,
}

impl ElevatedJob {
    pub(crate) fn org_plugins_for_current_user() -> Option<OrgPluginsJob> {
        let path = crate::config::paths::org_plugins_system()?;
        match std::env::var("USERNAME") {
            Ok(user) if !user.is_empty() => Some(OrgPluginsJob {
                path,
                grant_user: user,
            }),
            _ => {
                tracing::warn!("USERNAME not set; skipping org-plugins provisioning job");
                None
            },
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ElevatedResult {
    pub ok: bool,
    pub error: Option<String>,
}

pub(crate) fn perform_elevated_write(job_path: &str, result_path: &str) -> ExitCode {
    let outcome = run_job(job_path);
    let result = match &outcome {
        Ok(()) => ElevatedResult {
            ok: true,
            error: None,
        },
        Err(e) => ElevatedResult {
            ok: false,
            error: Some(e.to_string()),
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

#[derive(Debug, thiserror::Error)]
pub(crate) enum ElevateError {
    #[error("{action} {path}: {source}")]
    Io {
        action: &'static str,
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("decode staged job: {0}")]
    Decode(#[source] serde_json::Error),
    #[error("staged registry profile contained no policy values")]
    NoPolicyValues,
    #[error("policy: {0}")]
    Policy(#[source] crate::config::store::ConfigStoreError),
    #[error("spawn icacls: {0}")]
    Spawn(#[source] std::io::Error),
    #[error("icacls grant failed (exit {code:?}): {stderr}")]
    Icacls { code: Option<i32>, stderr: String },
}

fn io(
    action: &'static str,
    path: impl std::fmt::Display,
) -> impl FnOnce(std::io::Error) -> ElevateError {
    let path = path.to_string();
    move |source| ElevateError::Io {
        action,
        path,
        source,
    }
}

fn run_job(job_path: &str) -> Result<(), ElevateError> {
    let body = std::fs::read_to_string(job_path).map_err(io("read staged job", job_path))?;
    let job: ElevatedJob = serde_json::from_str(&body).map_err(ElevateError::Decode)?;
    if let Some(reg_path) = &job.reg_path {
        write_from_reg(reg_path)?;
    }
    if !job.clear_values.is_empty() {
        let names: Vec<&str> = job.clear_values.iter().map(String::as_str).collect();
        clear_managed_claude_policy(true, &names).map_err(ElevateError::Policy)?;
    }
    if !job.bridge_values.is_empty() {
        write_bridge_policy(true, &job.bridge_values).map_err(ElevateError::Policy)?;
    }
    if let Some(org) = &job.org_plugins {
        provision_org_plugins(&org.path, &org.grant_user)?;
    }
    for file in &job.managed_files {
        if let Some(parent) = file.dest.parent() {
            std::fs::create_dir_all(parent).map_err(io("create", parent.display()))?;
        }
        std::fs::copy(&file.staged, &file.dest).map_err(io("install", file.dest.display()))?;
    }
    for dest in &job.remove_files {
        match std::fs::remove_file(dest) {
            Ok(()) => {},
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {},
            Err(e) => return Err(io("remove", dest.display())(e)),
        }
    }
    Ok(())
}

fn write_from_reg(reg_path: &str) -> Result<(), ElevateError> {
    let body = std::fs::read_to_string(reg_path).map_err(io("read staged profile", reg_path))?;
    let entries = super::reg_values::parse_reg_entries(&body);
    if entries.is_empty() {
        return Err(ElevateError::NoPolicyValues);
    }
    write_managed_claude_policy(true, &entries).map_err(ElevateError::Policy)
}

pub(crate) fn provision_org_plugins(path: &Path, grant_user: &str) -> Result<(), ElevateError> {
    std::fs::create_dir_all(path).map_err(io("create org-plugins dir", path.display()))?;
    let grant_arg = format!("{grant_user}:(OI)(CI)M");
    let output = crate::winproc::no_window(&mut std::process::Command::new("icacls"))
        .arg(path.to_string_lossy().into_owned())
        .arg("/grant:r")
        .arg(&grant_arg)
        .arg("/T")
        .output()
        .map_err(ElevateError::Spawn)?;
    if !output.status.success() {
        return Err(ElevateError::Icacls {
            code: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    tracing::info!(path = %path.display(), user = grant_user, "org-plugins provisioned with user Modify grant");
    Ok(())
}

// Why: the job file used to be one fixed name. The first-run host-profile
// write and the first sync's org-plugins provisioning both staged to it within
// a second of each other; the second overwrote the first before its elevated
// child had read it, the child ran the org-plugins job twice, both callers
// read the same `ok` result, and the profile was logged as installed with no
// registry write behind it — Cowork then started with a gateway provider and
// no base URL. Every request now owns its own job and result file.
pub(crate) fn elevate_and_run(stage_dir: &Path, job: &ElevatedJob) -> std::io::Result<()> {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let exe = std::env::current_exe()?;
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let job_path = stage_dir.join(format!("elevated-job-{}-{seq}.json", std::process::id()));
    let body = serde_json::to_string(job).map_err(std::io::Error::other)?;
    std::fs::write(&job_path, body)?;
    let job_path = job_path.to_string_lossy().into_owned();
    let result_path = format!("{job_path}.result.json");
    tracing::info!(
        job_path,
        policy = job.reg_path.is_some(),
        org_plugins = job.org_plugins.is_some(),
        "requesting elevation for machine-wide Claude policy and org-plugins provisioning"
    );
    let outcome = run_elevated(
        &exe,
        &["__install-claude-policy", &job_path, result_path.as_str()],
    );
    let result = match outcome {
        ElevationOutcome::Declined => Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "administrator approval was declined — the managed Claude policy was not written",
        )),
        ElevationOutcome::Failed(msg) => Err(std::io::Error::other(msg)),
        ElevationOutcome::Completed { exit_code } => finish(&result_path, exit_code),
    };
    _ = std::fs::remove_file(&job_path);
    _ = std::fs::remove_file(&result_path);
    result
}

fn finish(result_path: &str, exit_code: u32) -> std::io::Result<()> {
    let detail = read_result(result_path);
    if exit_code == 0 && detail.as_ref().is_none_or(|r| r.ok) {
        return Ok(());
    }
    let message = detail
        .and_then(|r| r.error)
        .unwrap_or_else(|| format!("elevated install step failed (exit code {exit_code})"));
    Err(std::io::Error::other(message))
}

fn read_result(result_path: &str) -> Option<ElevatedResult> {
    let body = std::fs::read_to_string(result_path).ok()?;
    serde_json::from_str(&body).ok()
}
