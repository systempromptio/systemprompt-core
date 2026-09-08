//! The elevated child: runs one staged job and writes the versioned result
//! file the unelevated parent verifies against the steps it requested.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;
use std::process::ExitCode;

use super::{ElevateError, ElevatedJob, StagedJob, io, step};
use crate::config::store::{
    clear_managed_claude_policy, write_bridge_policy, write_managed_claude_policy,
};
use crate::install::elevated_protocol::{
    CompletedStep, ElevatedResult, ElevatedState, PROTOCOL_VERSION,
};

pub(crate) fn perform_elevated_write(job_path: &str, result_path: &str) -> ExitCode {
    match perform_job(job_path, result_path) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            tracing::error!(error = %e, job_path, "elevated installation failed");
            ExitCode::FAILURE
        },
    }
}

fn perform_job(job_path: &str, result_path: &str) -> std::io::Result<()> {
    let bytes = std::fs::read(job_path)?;
    let staged: StagedJob = serde_json::from_slice(&bytes).map_err(std::io::Error::other)?;
    if staged.version != PROTOCOL_VERSION {
        return Err(std::io::Error::other("unsupported elevated job protocol"));
    }
    let mut result = ElevatedResult {
        version: PROTOCOL_VERSION,
        job_id: staged.id,
        outcome: ElevatedState::Started,
    };
    write_result(result_path, &result, &staged.requester_sid)?;
    let mut steps = Vec::new();
    let outcome = run_job(&staged.job, &mut steps);
    result.outcome = match &outcome {
        Ok(()) => ElevatedState::Completed { steps },
        Err(e) => ElevatedState::Failed {
            steps,
            error: e.to_string(),
        },
    };
    write_result(result_path, &result, &staged.requester_sid)?;
    outcome.map_err(std::io::Error::other)
}

fn write_result(result_path: &str, result: &ElevatedResult, reader: &str) -> std::io::Result<()> {
    let json = serde_json::to_vec(result).map_err(std::io::Error::other)?;
    crate::fsutil::atomic_write_for_reader(Path::new(result_path), &json, reader)
}

fn run_job(job: &ElevatedJob, steps: &mut Vec<CompletedStep>) -> Result<(), ElevateError> {
    if let Some(reg_path) = &job.reg_path {
        let receipt = write_from_reg(reg_path)?;
        let mut completed = step("policy", reg_path);
        completed.policies.push(receipt);
        steps.push(completed);
    }
    if !job.clear_values.is_empty() {
        let names: Vec<&str> = job.clear_values.iter().map(String::as_str).collect();
        clear_managed_claude_policy(true, &names).map_err(ElevateError::Policy)?;
        steps.push(step("clear_policy", crate::cowork_compat::HKLM_POLICY_KEY));
    }
    if !job.bridge_values.is_empty() {
        let receipt =
            write_bridge_policy(true, &job.bridge_values).map_err(ElevateError::Policy)?;
        let mut completed = step(
            "bridge_policy",
            crate::config::store::bridge_policy_subkey(),
        );
        completed.policies.push(receipt);
        steps.push(completed);
    }
    if let Some(org) = &job.org_plugins {
        provision_org_plugins(&org.path, &org.grant_user)?;
        steps.push(step("provision", org.path.display()));
    }
    for file in &job.managed_files {
        install_managed_file(&file.staged, &file.dest)?;
        steps.push(step("install", file.dest.display()));
    }
    for dest in &job.remove_files {
        remove_verified(dest)?;
        steps.push(step("remove", dest.display()));
    }
    Ok(())
}

fn install_managed_file(staged: &Path, dest: &Path) -> Result<(), ElevateError> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(io("create", parent.display()))?;
    }
    let bytes = std::fs::read(staged).map_err(io("read staged file", staged.display()))?;
    crate::fsutil::atomic_write_0644(dest, &bytes).map_err(io("install and verify", dest.display()))
}

fn remove_verified(dest: &Path) -> Result<(), ElevateError> {
    match std::fs::remove_file(dest) {
        Ok(()) => {},
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {},
        Err(e) => return Err(io("remove", dest.display())(e)),
    }
    if dest
        .try_exists()
        .map_err(io("verify removal", dest.display()))?
    {
        return Err(io("verify removal", dest.display())(std::io::Error::other(
            "file still exists",
        )));
    }
    Ok(())
}

fn write_from_reg(
    reg_path: &str,
) -> Result<crate::config::store::verified::PolicyReceipt, ElevateError> {
    let body = std::fs::read_to_string(reg_path).map_err(io("read staged profile", reg_path))?;
    let entries = super::super::reg_values::parse_reg_entries(&body);
    if entries.is_empty() {
        return Err(ElevateError::NoPolicyValues);
    }
    let receipt = write_managed_claude_policy(true, &entries).map_err(ElevateError::Policy)?;
    Ok(receipt)
}

pub(crate) fn provision_org_plugins(path: &Path, grant_user: &str) -> Result<(), ElevateError> {
    std::fs::create_dir_all(path).map_err(io("create org-plugins dir", path.display()))?;
    let grant_arg = format!("*{grant_user}:(OI)(CI)M");
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
