//! Binding a native fixture to an actual execution session.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{Result, write_native_file};
use std::path::Path;
use systemprompt_evaluation::capabilities::VerifiedNativeTarget;
use systemprompt_evaluation::experiments::execution::ExecutionLimits;
use systemprompt_evaluation::experiments::{ClientKind, FrozenSettings};
use systemprompt_identifiers::{EvalExecutionId, ModelId, SessionId};
use systemprompt_scheduler::services::evaluator::adapters::{AdapterContext, AdapterInvocation};
use systemprompt_scheduler::services::evaluator::client::ClientPurpose;

pub(super) fn bind(
    adapter: &dyn systemprompt_scheduler::services::evaluator::adapters::NativeAdapter,
    kind: ClientKind,
    version: &str,
    root: &Path,
    identity: &Path,
) -> Result<()> {
    if !root.is_absolute() || !identity.is_absolute() || std::fs::metadata(identity)?.len() > 65536
    {
        return Err("Invalid live fixture identity file".into());
    }
    let mut plan: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("plan.json"))?)?;
    if plan["fixture_only"] != true
        || plan["automated_target_enabled"] != false
        || serde_json::from_value::<ClientKind>(plan["client"].clone())? != kind
        || plan["client_version"] != version
        || plan["adapter_version"] != adapter.adapter_version()
        || plan["executable"] != adapter.executable()
    {
        return Err("Live binding differs from pinned native fixture plan".into());
    }
    let image = plan["image_id"]
        .as_str()
        .and_then(|value| value.strip_prefix("sha256:"))
        .ok_or("Missing observed native image digest")?;
    if image.len() != 64 || !image.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("Invalid observed native image digest".into());
    }
    let frozen: FrozenSettings = serde_json::from_value(plan["frozen"].clone())?;
    let limits: ExecutionLimits = serde_json::from_value(plan["limits"].clone())?;
    let model: ModelId = serde_json::from_value(plan["model"].clone())?;
    let purpose = match plan["scenario"].as_str() {
        Some("judge") => ClientPurpose::Judge,
        Some("suggestion") => ClientPurpose::Suggestion,
        Some(
            "mcp" | "candidate" | "baseline" | "credentials" | "forbidden" | "network" | "failure"
            | "cancel" | "attempt-bound" | "output-bound" | "deadline",
        ) => ClientPurpose::Execution,
        _ => return Err("Unknown bound fixture purpose".into()),
    };
    let expected=adapter.arguments(&AdapterInvocation{model:&model,limits:&limits,purpose,prompt:"Execute the native acceptance case using only the provided tool instructions. Report the observed fixture evidence. Do not access external services."})?.into_iter().map(|arg|arg.into_string().map_err(|_|"Invalid native argument")).collect::<std::result::Result<Vec<_>,_>>()?;
    if serde_json::to_value(expected)? != plan["argv"] {
        return Err("Bound native arguments differ from prepared purpose/limits/model".into());
    }
    let identity: serde_json::Value = serde_json::from_slice(&std::fs::read(identity)?)?;
    let text = |key: &str| {
        identity[key]
            .as_str()
            .ok_or("Missing issued execution identity")
    };
    let session = SessionId::new(text("session_id")?);
    let execution = EvalExecutionId::new(text("execution_id")?);
    let target = VerifiedNativeTarget {
        client: kind,
        platform: "linux".to_owned(),
        architecture: std::env::consts::ARCH.to_owned(),
        client_version: version.to_owned(),
        adapter_version: adapter.adapter_version().to_owned(),
        image_digest: image.to_owned(),
        executable_digest: "f".repeat(64),
        native_isolation_evidence_digest: "f".repeat(64),
        native_metering_evidence_digest: "f".repeat(64),
    };
    let archive = adapter.configuration(&AdapterContext {
        relay_url: "http://eval-relay-native:8090",
        execution_token: text("execution_token")?,
        session_id: &session,
        execution_id: &execution,
        target: &target,
        frozen: &frozen,
    })?;
    archive.validate()?;
    for (name, file) in &archive.files {
        let destination = root.join(name);
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)?;
        }
        write_native_file(&destination, &file.bytes, file.executable)?;
    }
    plan["configuration_digest"] = serde_json::json!(archive.digest()?);
    plan["live_execution_id"] = serde_json::json!(execution);
    plan["live_session_id"] = serde_json::json!(session);
    std::fs::write(root.join("plan.json"), serde_json::to_vec_pretty(&plan)?)?;
    Ok(())
}
