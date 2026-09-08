//! Versioned UAC jobs for machine policy, org-plugins and administrator-owned
//! files.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod child;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::elevated_protocol::{CompletedStep, ElevatedResult, PROTOCOL_VERSION};
use crate::winproc::{ElevationOutcome, run_elevated};

pub(crate) use self::child::{perform_elevated_write, provision_org_plugins};

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

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct OrgPluginsJob {
    pub path: PathBuf,
    // Why: UAC can run the child as a different admin; capture the grantee before elevation.
    pub grant_user: String,
}

impl ElevatedJob {
    pub(crate) fn org_plugins_for_current_user() -> std::io::Result<OrgPluginsJob> {
        let path = crate::config::paths::org_plugins_system()
            .ok_or_else(|| std::io::Error::other("system org-plugins path unresolvable"))?;
        let user = crate::windows_acl::current_sid()?;
        Ok(OrgPluginsJob {
            path,
            grant_user: user,
        })
    }
}

#[derive(Debug, Serialize)]
struct StagedJobRef<'a> {
    version: u32,
    requester_sid: String,
    id: uuid::Uuid,
    job: &'a ElevatedJob,
}

#[derive(Debug, Deserialize)]
struct StagedJob {
    version: u32,
    requester_sid: String,
    id: uuid::Uuid,
    job: ElevatedJob,
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

#[derive(Debug)]
#[must_use]
pub(crate) struct ElevatedReceipt {
    steps: Vec<CompletedStep>,
}
impl ElevatedReceipt {
    pub(crate) fn require(&self, operation: &str, target: &Path) -> std::io::Result<()> {
        let target = target.display().to_string();
        if self
            .steps
            .iter()
            .any(|step| step.operation == operation && step.target == target)
        {
            Ok(())
        } else {
            Err(std::io::Error::other(format!(
                "elevated receipt lacks {operation} {target}"
            )))
        }
    }
    pub(crate) fn steps(&self) -> &[CompletedStep] {
        &self.steps
    }
}

pub(crate) fn elevate_and_run(
    stage_dir: &Path,
    job: &ElevatedJob,
) -> std::io::Result<ElevatedReceipt> {
    let stage = tempfile::Builder::new()
        .prefix("elevated-job-")
        .tempdir_in(stage_dir)?;
    let id = uuid::Uuid::new_v4();
    let body = serde_json::to_vec(&StagedJobRef {
        version: PROTOCOL_VERSION,
        requester_sid: crate::windows_acl::current_sid()?,
        id,
        job,
    })
    .map_err(std::io::Error::other)?;
    let job_path = stage.path().join("job.json");
    let result_path = stage.path().join("result.json");
    crate::fsutil::atomic_write_0600(&job_path, &body)?;
    let outcome = run_elevated(
        &std::env::current_exe()?,
        &[
            "__install-claude-policy",
            &job_path.to_string_lossy(),
            &result_path.to_string_lossy(),
        ],
    );
    match outcome {
        ElevationOutcome::Declined => Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "administrator approval declined; installation was not completed",
        )),
        ElevationOutcome::Failed(message) => Err(std::io::Error::other(message)),
        ElevationOutcome::Completed { exit_code } => {
            verify_completed(job, id, exit_code, &result_path)
        },
    }
}

fn verify_completed(
    job: &ElevatedJob,
    id: uuid::Uuid,
    exit_code: u32,
    result_path: &Path,
) -> std::io::Result<ElevatedReceipt> {
    let bytes = std::fs::read(result_path).map_err(|e| {
        std::io::Error::other(format!(
            "elevated result {} cannot be read: {e}",
            result_path.display()
        ))
    })?;
    let result: ElevatedResult = serde_json::from_slice(&bytes).map_err(std::io::Error::other)?;
    let steps = result
        .verify(id, exit_code, &expected_steps(job))
        .map_err(std::io::Error::other)?;
    if let Some(org) = &job.org_plugins {
        crate::windows_acl::verify_modify_tree(&org.path)?;
    }
    for file in &job.managed_files {
        crate::fsutil::verify_contents(&file.dest, &std::fs::read(&file.staged)?)?;
    }
    for path in &job.remove_files {
        if path.try_exists()? {
            return Err(std::io::Error::other(format!(
                "{} still exists after elevated removal",
                path.display()
            )));
        }
    }
    Ok(ElevatedReceipt { steps })
}

fn step(operation: &str, target: impl std::fmt::Display) -> CompletedStep {
    CompletedStep {
        operation: operation.to_owned(),
        target: target.to_string(),
        policies: Vec::new(),
    }
}

fn expected_steps(job: &ElevatedJob) -> Vec<CompletedStep> {
    let mut steps = Vec::new();
    if let Some(path) = &job.reg_path {
        steps.push(step("policy", path));
    }
    if !job.clear_values.is_empty() {
        steps.push(step("clear_policy", crate::cowork_compat::HKLM_POLICY_KEY));
    }
    if !job.bridge_values.is_empty() {
        steps.push(step(
            "bridge_policy",
            crate::config::store::bridge_policy_subkey(),
        ));
    }
    if let Some(org) = &job.org_plugins {
        steps.push(step("provision", org.path.display()));
    }
    steps.extend(
        job.managed_files
            .iter()
            .map(|file| step("install", file.dest.display())),
    );
    steps.extend(
        job.remove_files
            .iter()
            .map(|path| step("remove", path.display())),
    );
    steps
}
