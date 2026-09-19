//! The unelevated side of the policy writer: installing it (from the
//! elevated install), asking whether it is usable, and handing it a request
//! and reading the answer back.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![cfg(target_os = "windows")]

use std::io;
use std::path::Path;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use super::{
    BIN_SDDL, Layout, PolicyWriteRequest, PolicyWriterError, REQUEST_VERSION, RESULT_TIMEOUT_SECS,
    derive_policy, expected_steps, task,
};
use crate::config::store::{self, ConfigStore, PolicyHive, PolicyTarget};
use crate::install::elevated_protocol::{CompletedStep, ElevatedResult};

const STAMP_FILE: &str = "writer.json";

// Why: the copy under ProgramData is refreshed only by an elevated install,
// so the running bridge may be newer than the writer. The stamp says which
// request protocol the copy speaks; a mismatch is a refusal, never a guess.
#[derive(Debug, Serialize, Deserialize)]
struct WriterStamp {
    bridge_version: String,
    request_version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WriterStatus {
    Ready,
    NotRegistered,
    Unavailable(String),
}

fn io_ctx(context: impl Into<String>) -> impl FnOnce(io::Error) -> PolicyWriterError {
    let context = context.into();
    move |source| PolicyWriterError::Io { context, source }
}

pub(crate) fn layout() -> Result<Layout, PolicyWriterError> {
    crate::windows_acl::program_data()
        .map(|root| Layout::under(&root))
        .map_err(io_ctx("resolve machine ProgramData"))
}

pub(crate) fn install(exe: &Path) -> Result<Vec<String>, PolicyWriterError> {
    if !crate::winproc::is_elevated() {
        return Err(PolicyWriterError::Unavailable(
            "the writer is registered by an elevated install only".to_owned(),
        ));
    }
    require_anchor()?;
    let layout = layout()?;
    let _directories = super::paths::secure_directories(&layout, true)
        .map_err(io_ctx("secure machine writer directories"))?;
    let bytes = std::fs::read(exe).map_err(io_ctx(format!("read {}", exe.display())))?;
    crate::fsutil::atomic_write_0644(&layout.binary, &bytes)
        .map_err(io_ctx(format!("install {}", layout.binary.display())))?;
    let stamp = serde_json::to_vec(&WriterStamp {
        bridge_version: crate::brand::brand().version.to_owned(),
        request_version: REQUEST_VERSION,
    })
    .map_err(|e| io_ctx("encode writer stamp")(io::Error::other(e)))?;
    crate::fsutil::atomic_write_0644(&layout.bin.join(STAMP_FILE), &stamp)
        .map_err(io_ctx("write writer stamp"))?;
    for path in [&layout.binary, &layout.bin.join(STAMP_FILE)] {
        let _file = crate::windows_acl::lock_machine_path(path, false)
            .map_err(io_ctx("verify machine file owner"))?;
        crate::windows_acl::apply_directory_sddl(path, BIN_SDDL)
            .map_err(io_ctx("protect machine file"))?;
    }
    task::register(&layout.binary, &layout.root).map_err(io_ctx("register writer task"))?;
    Ok(vec![
        format!("policy writer: {}", layout.binary.display()),
        format!(
            "scheduled task: {} (SYSTEM, on request; users may run it)",
            super::task_name()
        ),
    ])
}

pub(crate) fn remove() -> Result<Vec<String>, PolicyWriterError> {
    task::remove().map_err(io_ctx("remove writer task"))?;
    let layout = layout()?;
    if layout.root.exists() {
        std::fs::remove_dir_all(&layout.root)
            .map_err(io_ctx(format!("remove {}", layout.root.display())))?;
    }
    Ok(vec![format!(
        "removed policy writer {}",
        super::task_name()
    )])
}

// Why: a status is what the bridge trusts before it delegates a write, so
// every leg is checked from what is on disk and in the scheduler: the task,
// the binary it runs, the DACLs that keep users out of it, and the protocol
// the copy speaks.
pub(crate) fn status() -> WriterStatus {
    let layout = match layout() {
        Ok(layout) => layout,
        Err(e) => return WriterStatus::Unavailable(e.to_string()),
    };
    match task::exists() {
        Ok(true) => {},
        Ok(false) => return WriterStatus::NotRegistered,
        Err(e) => return WriterStatus::Unavailable(format!("task query: {e}")),
    }
    if let Err(e) = task::verify_registered(&layout.binary) {
        return WriterStatus::Unavailable(e.to_string());
    }
    let _directories = match super::paths::secure_directories(&layout, false) {
        Ok(handles) => handles,
        Err(e) => return WriterStatus::Unavailable(e.to_string()),
    };
    for path in [&layout.binary, &layout.bin.join(STAMP_FILE)] {
        if let Err(e) = crate::windows_acl::lock_machine_path(path, false)
            .and_then(|_file| crate::windows_acl::verify_directory_sddl(path, BIN_SDDL))
        {
            return WriterStatus::Unavailable(format!("{}: {e}", path.display()));
        }
    }
    match read_stamp(&layout) {
        Ok(stamp) if stamp.request_version == REQUEST_VERSION => WriterStatus::Ready,
        Ok(stamp) => WriterStatus::Unavailable(format!(
            "the installed writer ({}) speaks request protocol {}; this bridge speaks {REQUEST_VERSION} — re-run install --apply as Administrator",
            stamp.bridge_version, stamp.request_version
        )),
        Err(e) => WriterStatus::Unavailable(format!("writer stamp: {e}")),
    }
}

fn read_stamp(layout: &Layout) -> io::Result<WriterStamp> {
    let bytes = std::fs::read(layout.bin.join(STAMP_FILE))?;
    serde_json::from_slice(&bytes).map_err(io::Error::other)
}

fn require_anchor() -> Result<(), PolicyWriterError> {
    read_anchor(store::managed_policy_store().as_ref())?
        .map(|_| ())
        .ok_or(PolicyWriterError::NoAnchor)
}

// Why: the machine hive alone; the user hive and the environment override
// are the user's to write, and the writer must not be steered by either.
pub(crate) fn read_anchor(
    store: &dyn ConfigStore,
) -> Result<Option<crate::config::TrustRecord>, PolicyWriterError> {
    let doc = store.read_policy_document(
        PolicyHive::Machine,
        PolicyTarget::Bridge,
        &[store::MANIFEST_TRUST_KEY],
    )?;
    let Some(store::PolicyDocumentValue::Str(raw)) = doc.get(store::MANIFEST_TRUST_KEY) else {
        return Ok(None);
    };
    let record: crate::config::TrustRecord = serde_json::from_str(raw)
        .map_err(|e| crate::config::TrustError::InvalidPolicy(e.to_string()))?;
    let gateway = systemprompt_identifiers::ValidatedUrl::try_new(record.gateway.as_str())
        .map_err(|e| crate::config::TrustError::InvalidPolicy(format!("gateway: {e}")))?;
    Ok(Some(crate::config::TrustRecord::new(
        &gateway,
        record.key.as_str(),
        crate::config::PinSource::Policy,
    )?))
}

pub(crate) fn write_policy(
    request: &PolicyWriteRequest,
) -> Result<Vec<CompletedStep>, PolicyWriterError> {
    match status() {
        WriterStatus::Ready => {},
        WriterStatus::NotRegistered => return Err(PolicyWriterError::NotRegistered),
        WriterStatus::Unavailable(why) => return Err(PolicyWriterError::Unavailable(why)),
    }
    let layout = layout()?;
    let request_path = layout.request_path(request.job_id);
    let result_path = layout.result_path(request.job_id);
    let body = serde_json::to_vec(request)
        .map_err(|e| io_ctx("encode policy write request")(io::Error::other(e)))?;
    crate::fsutil::atomic_write_0600(&request_path, &body)
        .map_err(io_ctx(format!("stage {}", request_path.display())))?;
    task::run().map_err(io_ctx("start writer task"))?;
    let result = await_result(&result_path)?;
    let steps = result.verify(request.job_id, 0, &expected_steps())?;
    verify_hive(request)?;
    crate::fsutil::remove_leftover_file(&result_path);
    Ok(steps)
}

fn await_result(result_path: &Path) -> Result<ElevatedResult, PolicyWriterError> {
    let deadline = Instant::now() + Duration::from_secs(RESULT_TIMEOUT_SECS);
    loop {
        match std::fs::read(result_path) {
            Ok(bytes) => {
                let result: ElevatedResult = serde_json::from_slice(&bytes)
                    .map_err(|e| io_ctx("decode writer result")(io::Error::other(e)))?;
                if !matches!(
                    result.outcome,
                    crate::install::elevated_protocol::ElevatedState::Started
                ) {
                    return Ok(result);
                }
            },
            Err(e) if e.kind() == io::ErrorKind::NotFound => {},
            Err(e) => return Err(io_ctx(format!("read {}", result_path.display()))(e)),
        }
        if Instant::now() >= deadline {
            return Err(PolicyWriterError::Timeout);
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

// Why: the writer's own receipt is evidence of what it did; whether the hive
// now holds what this bridge meant is read back independently, from the
// same derivation the writer used, before the write is reported as done.
fn verify_hive(request: &PolicyWriteRequest) -> Result<(), PolicyWriterError> {
    let manifest = crate::gateway::manifest::decode_payload(&request.envelope)?;
    let store = store::managed_policy_store();
    let existing_models = store.read_policy_document(
        PolicyHive::Machine,
        PolicyTarget::Claude,
        &["inferenceModels"],
    )?;
    let existing_models = match existing_models.get("inferenceModels") {
        Some(store::PolicyDocumentValue::Str(models)) => Some(models.clone()),
        _ => None,
    };
    let expected = derive_policy(request, &manifest, existing_models)?;
    let names: Vec<&str> = expected.iter().map(|(name, _, _)| *name).collect();
    let actual = store.read_policy_document(PolicyHive::Machine, PolicyTarget::Claude, &names)?;
    let differing: Vec<&str> = expected
        .iter()
        .filter(|(name, _, value)| {
            actual.get(*name) != Some(&store::PolicyDocumentValue::Str(value.clone()))
        })
        .map(|(name, _, _)| *name)
        .collect();
    if differing.is_empty() {
        return Ok(());
    }
    Err(PolicyWriterError::Unavailable(format!(
        "reported success but the machine policy still differs for {}",
        differing.join(", ")
    )))
}
