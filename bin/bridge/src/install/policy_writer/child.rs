//! The SYSTEM side of the policy writer: the task's action. It processes
//! every request in the inbox, acts only on a request file owned by the
//! account the machine policy is bound to, believes nothing a request says
//! until the manifest it carries has verified against the machine anchor and
//! is newer than the last one accepted, and answers each requester in the
//! outbox.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![cfg(target_os = "windows")]

use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use super::spool::read_anchor;
use super::{
    AccountSid, AcceptedEnvelope, Layout, PolicyWriteRequest, PolicyWriterError, binding,
    derive_policy, machine_state, verify_against_anchor,
};
use crate::config::store::{self, PolicyHive, PolicyTarget};
use crate::install::elevated_protocol::{
    CompletedStep, ElevatedResult, ElevatedState, PROTOCOL_VERSION,
};

pub(crate) fn perform_task(spool_root: &str) -> ExitCode {
    if !crate::winproc::is_elevated() {
        tracing::error!(target: "bridge::policy_writer", "the policy writer must run elevated");
        return ExitCode::FAILURE;
    }
    // Why: the root is recomputed from the machine's ProgramData rather than
    // taken from the argument; the argument only has to agree, so a task
    // whose action was edited to another directory refuses to run.
    let layout = match super::spool::layout() {
        Ok(layout) => layout,
        Err(e) => {
            tracing::error!(target: "bridge::policy_writer", error = %e, "cannot resolve writer root");
            return ExitCode::FAILURE;
        },
    };
    if Path::new(spool_root) != layout.root {
        tracing::error!(
            target: "bridge::policy_writer",
            given = spool_root,
            expected = %layout.root.display(),
            "spool root argument does not name this computer's policy-writer root"
        );
        return ExitCode::FAILURE;
    }
    let _directories = match super::paths::secure_directories(&layout, false) {
        Ok(handles) => handles,
        Err(e) => {
            tracing::error!(target: "bridge::policy_writer", error = %e, "unsafe writer directories");
            return ExitCode::FAILURE;
        },
    };
    let mut failed = false;
    for request_path in pending_requests(&layout) {
        if let Err(e) = process(&layout, &request_path) {
            failed = true;
            tracing::error!(
                target: "bridge::policy_writer",
                request = %request_path.display(),
                error = %e,
                "policy write request failed"
            );
        }
        crate::fsutil::remove_leftover_file(&request_path);
    }
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn pending_requests(layout: &Layout) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(&layout.inbox) else {
        return Vec::new();
    };
    let mut out: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("request-"))
                && path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
        })
        .collect();
    out.sort();
    out
}

fn process(layout: &Layout, request_path: &Path) -> Result<(), PolicyWriterError> {
    let (request, owner) = super::request::read_owned_request(request_path)?;
    let result_path = layout.result_path(&request.job_id);
    let mut result = ElevatedResult {
        version: PROTOCOL_VERSION,
        job_id: request.job_id.clone(),
        outcome: ElevatedState::Started,
    };
    write_result(&result_path, &result, &owner)?;
    let mut steps = Vec::new();
    let outcome = apply(layout, &request, &owner, &mut steps);
    result.outcome = match &outcome {
        Ok(()) => ElevatedState::Completed { steps },
        Err(e) => ElevatedState::Failed {
            steps,
            error: e.to_string(),
        },
    };
    write_result(&result_path, &result, &owner)?;
    outcome
}

fn apply(
    layout: &Layout,
    request: &PolicyWriteRequest,
    owner: &AccountSid,
    steps: &mut Vec<CompletedStep>,
) -> Result<(), PolicyWriterError> {
    let bound = machine_state::read_binding(layout)?;
    binding::authorize(owner, &bound)?;
    let store = store::managed_policy_store();
    let anchors = read_anchor(store.as_ref())?;
    let requested = systemprompt_identifiers::ValidatedUrl::try_new(&request.gateway)
        .map_err(|e| PolicyWriterError::Anchor(crate::config::TrustError::GatewayInvalid(e)))?;
    let anchor = crate::config::trust::select_pin(anchors, &requested)?
        .ok_or(PolicyWriterError::NoAnchor)?;
    let manifest = verify_against_anchor(request, &anchor)?;
    let digest = binding::envelope_digest(&request.envelope)?;
    let last = machine_state::read_accepted(layout)?;
    let admission = binding::admit(&manifest, &digest, last.as_ref(), chrono::Utc::now())?;
    if admission == binding::Admission::Fresh {
        machine_state::write_accepted(
            layout,
            &AcceptedEnvelope {
                manifest_version: manifest.manifest_version.clone(),
                envelope_digest: digest,
            },
        )?;
    }
    let existing = store.read_policy_document(
        PolicyHive::Machine,
        PolicyTarget::Claude,
        &["inferenceModels"],
    )?;
    let existing_models = match existing.get("inferenceModels") {
        Some(store::PolicyDocumentValue::Str(models)) => Some(models.clone()),
        _ => None,
    };
    let values = derive_policy(&bound, request, &manifest, existing_models)?;
    let withheld = store::policy_apply::withheld_keys(&values);
    let mut names: Vec<&str> = values.iter().map(|(name, _, _)| *name).collect();
    names.extend(&withheld);
    let current = store.read_policy_document(PolicyHive::Machine, PolicyTarget::Claude, &names)?;
    let differs = values.iter().any(|(name, _, value)| {
        current.get(*name) != Some(&store::PolicyDocumentValue::Str(value.clone()))
    }) || withheld.iter().any(|name| current.contains_key(*name));
    let outcome = binding::ApplyOutcome::of(admission, differs);
    // Why: the result protocol requires a receipt for the policy step, so an
    // unchanged re-apply still runs the verified write; it rewrites the same
    // values and the outcome records that nothing differed.
    let receipt =
        store::policy_apply::apply_claude_policy(store.as_ref(), PolicyHive::Machine, &values)?;
    tracing::info!(
        target: "bridge::policy_writer",
        job = %request.job_id,
        servers = manifest.managed_mcp_servers.len(),
        receipt = %receipt.describe(),
        outcome = ?outcome,
        "machine policy written from a verified manifest"
    );
    steps.push(CompletedStep {
        operation: "policy".to_owned(),
        target: crate::cowork_compat::HKLM_POLICY_KEY.to_owned(),
        policies: vec![receipt],
    });
    Ok(())
}

fn write_result(
    path: &Path,
    result: &ElevatedResult,
    reader: &AccountSid,
) -> Result<(), PolicyWriterError> {
    let json = serde_json::to_vec(result).map_err(|e| PolicyWriterError::Io {
        context: "encode writer result".to_owned(),
        source: io::Error::other(e),
    })?;
    crate::fsutil::atomic_write_for_reader(path, &json, reader.as_str()).map_err(|source| {
        PolicyWriterError::Io {
            context: format!("write {}", path.display()),
            source,
        }
    })
}
