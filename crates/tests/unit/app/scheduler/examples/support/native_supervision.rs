//! Native fixture execution through the production container supervision
//! primitive.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;
use std::time::{Duration, Instant};
use systemprompt_evaluation::experiments::ClientKind;
use systemprompt_evaluation::experiments::execution::ExecutionLimits;
use systemprompt_identifiers::ModelId;
use systemprompt_scheduler::SchedulerResult;
use systemprompt_scheduler::services::evaluator::client::NativeClient;
use systemprompt_scheduler::services::evaluator::container::{ClientVerifier, ContainerLaunch};

#[derive(Debug)]
struct FixturePreverified;
impl ClientVerifier for FixturePreverified {
    fn verify(&self, _: &ContainerLaunch, _: &NativeClient) -> SchedulerResult<()> {
        // The enclosing native harness probes image config, executable bytes and
        // exact parsed version before this private fixture action. Production
        // admission still uses PinnedClientVerifier and the empty reviewed registry.
        Ok(())
    }
}

pub(super) fn run(kind: ClientKind, root: &Path, settings: &Path) -> super::Result<()> {
    if !root.is_absolute() || !settings.is_absolute() {
        return Err("Native supervision requires absolute fixture paths".into());
    }
    let plan: serde_json::Value = serde_json::from_slice(&std::fs::read(root.join("plan.json"))?)?;
    let settings: serde_json::Value = serde_json::from_slice(&std::fs::read(settings)?)?;
    let scenario = plan["scenario"].as_str().ok_or("Missing scenario")?;
    if !matches!(scenario, "output-bound" | "cancel" | "deadline")
        || plan["fixture_only"] != true
        || plan["automated_target_enabled"] != false
        || serde_json::from_value::<ClientKind>(plan["client"].clone())? != kind
    {
        return Err("Unsupported supervised fixture".into());
    }
    let limits: ExecutionLimits = serde_json::from_value(plan["limits"].clone())?;
    let text = |key: &str| {
        settings[key]
            .as_str()
            .ok_or("Missing supervised launch identity")
    };
    let client = NativeClient::builder(
        kind,
        ModelId::new(plan["model"].as_str().ok_or("Missing model")?),
    )
    .limits(limits)
    .build()?;
    let launch = ContainerLaunch::builder(text("docker")?.into(), root.to_path_buf())
        .image(text("image")?.to_owned())
        .network(text("network")?.to_owned())
        .name(text("name")?.to_owned())
        .ownership("native-proof", "native-proof")
        .output_stem("supervised")
        .verifier(std::sync::Arc::new(FixturePreverified))
        .build()?;
    let mut execution = launch.start(&client, "Execute the native acceptance case using only the provided tool instructions. Report the observed fixture evidence. Do not access external services.")?;
    let started = Instant::now();
    let mut cancelled = false;
    let mut limit_rejected = false;
    loop {
        if scenario == "cancel" {
            let audit =
                std::fs::read_to_string(root.join("evidence/provider.jsonl")).unwrap_or_default();
            if audit
                .lines()
                .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
                .any(|event| event["kind"] == "provider")
            {
                execution.cancel()?;
                cancelled = true;
                break;
            }
        }
        match execution.poll() {
            Ok(Some(_)) => break,
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(error) => {
                if !error
                    .to_string()
                    .contains("exceeded execution time, output, or writable-storage limit")
                {
                    return Err(error.into());
                }
                limit_rejected = true;
                break;
            },
        }
    }
    let (stdout, stderr) = execution.output_paths();
    let observed_bytes = std::fs::metadata(stdout)?.len() + std::fs::metadata(stderr)?.len();
    let elapsed = started.elapsed().as_millis();
    let output_rejected = scenario == "output-bound"
        && limit_rejected
        && observed_bytes > limits.max_artifact_bytes
        && elapsed < u128::from(limits.active_timeout_seconds) * 1000;
    let deadline_rejected = scenario == "deadline"
        && limit_rejected
        && observed_bytes <= limits.max_artifact_bytes
        && elapsed >= u128::from(limits.active_timeout_seconds) * 1000;
    if !(cancelled || output_rejected || deadline_rejected)
        || elapsed > u128::from(limits.active_timeout_seconds + 10) * 1000
        || observed_bytes > 16 * 1024 * 1024
    {
        return Err("Native runtime supervision did not establish the requested boundary".into());
    }
    std::fs::copy(stdout, root.join("stdout.jsonl"))?;
    std::fs::copy(stderr, root.join("stderr.log"))?;
    std::fs::write(
        root.join("supervision.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "primitive":"ContainerExecution", "cancelled":cancelled,
            "output_bound_rejected":output_rejected,"deadline_rejected":deadline_rejected,
            "configured_output_bytes":limits.max_artifact_bytes,"observed_output_bytes":observed_bytes,
            "elapsed_milliseconds":elapsed,"cleanup_acknowledged":true
        }))?,
    )?;
    Ok(())
}
