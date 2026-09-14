//! Materializes real adapter contracts for external, offline native acceptance.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;
use systemprompt_evaluation::capabilities::VerifiedNativeTarget;
use systemprompt_evaluation::experiments::execution::ExecutionLimits;
use systemprompt_evaluation::experiments::{ClientKind, FrozenCostEnvelope, FrozenSettings};
use systemprompt_identifiers::{EvalExecutionId, ModelId, SessionId};
use systemprompt_scheduler::services::evaluator::adapters::{
    AdapterContext, AdapterInvocation, adapter, normalize_evidence,
};
use systemprompt_scheduler::services::evaluator::client::ClientPurpose;

#[path = "support/native_binding.rs"]
mod native_binding;
#[path = "support/native_supervision.rs"]
mod native_supervision;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
fn client(value: &str) -> Result<(ClientKind, &'static str)> {
    Ok(match value {
        "claude-code" => (ClientKind::ClaudeCode, "2.1.270"),
        "open-code" => (ClientKind::Opencode, "1.18.29"),
        "codex" => (ClientKind::Codex, "0.154.0"),
        "hermes" => (ClientKind::Hermes, "0.21.3"),
        _ => return Err("Unsupported native fixture client".into()),
    })
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 4 {
        return Err("usage: native_adapter_fixture prepare|normalize|version|bind client input-path output-or-scenario".into());
    }
    let (kind, version) = client(&args[1])?;
    let adapter = adapter(kind)?;
    match args[0].as_str() {
        "supervise" => native_supervision::run(kind, Path::new(&args[2]), Path::new(&args[3])),
        "bind" => native_binding::bind(
            adapter,
            kind,
            version,
            Path::new(&args[2]),
            Path::new(&args[3]),
        ),
        "prepare" => prepare(adapter, kind, version, Path::new(&args[2]), &args[3]),
        "normalize" => {
            let metadata = std::fs::metadata(&args[2])?;
            if metadata.len() > 16 * 1024 * 1024 {
                return Err("Native output exceeds bound".into());
            }
            let output = normalize_evidence(adapter, &std::fs::read(&args[2])?);
            std::fs::write(&args[3], serde_json::to_vec_pretty(&output)?)?;
            Ok(())
        },
        "version" => {
            let bytes = std::fs::read(&args[2])?;
            let parsed = adapter.parse_version(&bytes)?;
            if parsed != version {
                return Err("Native fixture version differs from adapter pin".into());
            }
            std::fs::write(&args[3], parsed)?;
            Ok(())
        },
        _ => Err("Unknown native fixture action".into()),
    }
}
fn prepare(
    adapter: &dyn systemprompt_scheduler::services::evaluator::adapters::NativeAdapter,
    kind: ClientKind,
    version: &str,
    root: &Path,
    scenario: &str,
) -> Result<()> {
    if !root.is_absolute()
        || !matches!(
            scenario,
            "mcp"
                | "candidate"
                | "baseline"
                | "credentials"
                | "forbidden"
                | "network"
                | "failure"
                | "cancel"
                | "judge"
                | "suggestion"
                | "attempt-bound"
                | "output-bound"
                | "deadline"
        )
    {
        return Err("Invalid native fixture destination or scenario".into());
    }
    std::fs::create_dir(root)?;
    let target = VerifiedNativeTarget {
        client: kind,
        platform: "linux".to_owned(),
        architecture: std::env::consts::ARCH.to_owned(),
        client_version: version.to_owned(),
        adapter_version: adapter.adapter_version().to_owned(),
        image_digest: "f".repeat(64),
        executable_digest: "f".repeat(64),
        native_isolation_evidence_digest: "f".repeat(64),
        native_metering_evidence_digest: "f".repeat(64),
    };
    let frozen = frozen();
    let session = SessionId::new("native-acceptance-session");
    let execution = EvalExecutionId::new("native-acceptance-execution");
    let context = AdapterContext {
        relay_url: "http://eval-relay-native:8090",
        execution_token: "spexec_native_fixture",
        session_id: &session,
        execution_id: &execution,
        target: &target,
        frozen: &frozen,
    };
    let archive = adapter.configuration(&context)?;
    for (name, file) in &archive.files {
        let path = root.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        write_native_file(&path, &file.bytes, file.executable)?;
    }
    let skill = root
        .join("home")
        .join(adapter.skill_directory())
        .join("fixture");
    std::fs::create_dir_all(&skill)?;
    if scenario != "baseline" {
        write_native_file(
            &skill.join("SKILL.md"),
            b"---\nname: fixture\ndescription: Native acceptance fixture\n---\nNATIVE_SKILL_SENTINEL\n", false,
        )?;
    }
    std::fs::create_dir_all(root.join("home/work"))?;
    write_native_file(
        &root.join("home/work/case.txt"),
        b"NATIVE_WORKSPACE_SENTINEL",
        false,
    )?;
    let purpose = match scenario {
        "judge" => ClientPurpose::Judge,
        "suggestion" => ClientPurpose::Suggestion,
        _ => ClientPurpose::Execution,
    };
    let limits = ExecutionLimits {
        max_turns: if scenario == "attempt-bound" { 1 } else { 4 },
        max_output_tokens: 512,
        active_timeout_seconds: if scenario == "deadline" { 5 } else { 30 },
        max_artifact_bytes: if scenario == "output-bound" {
            65536
        } else {
            16 * 1024 * 1024
        },
    };
    let model = ModelId::new(if kind == ClientKind::ClaudeCode {
        "claude-sonnet-4-6"
    } else {
        "gpt-5"
    });
    let prompt = "Execute the native acceptance case using only the provided tool instructions. Report the observed fixture evidence. Do not access external services.";
    let argv = adapter
        .arguments(&AdapterInvocation {
            model: &model,
            limits: &limits,
            purpose,
            prompt,
        })?
        .into_iter()
        .map(|arg| {
            arg.into_string()
                .map_err(|_| "Invalid native UTF-8 argument")
        })
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let meta = serde_json::json!({"fixture_only":true,"automated_target_enabled":false,"gateway_metering_verified":false,"client":adapter.client(),"client_version":version,"adapter_version":adapter.adapter_version(),"executable":adapter.executable(),"argv":argv,"version_arguments":adapter.version_arguments().into_iter().map(|arg|arg.into_string().expect("version arguments are UTF-8")).collect::<Vec<_>>(),"skill_path":format!("/home/tester/{}/fixture/SKILL.md",adapter.skill_directory()),"scenario":scenario,"model":model,"limits":limits,"configuration_digest":archive.digest()?,"frozen":frozen});
    std::fs::write(root.join("plan.json"), serde_json::to_vec_pretty(&meta)?)?;
    Ok(())
}

fn frozen() -> FrozenSettings {
    FrozenSettings {
        provider_prices_digest: "a".repeat(64),
        tool_configuration_digest: "b".repeat(64),
        fixture_clock: "2026-09-14T00:00:00Z".to_owned(),
        fixture_timezone: "UTC".to_owned(),
        permissions_digest: "c".repeat(64),
        dataset_digest: "d".repeat(64),
        rubric_digest: "e".repeat(64),
        cost_envelope: FrozenCostEnvelope {
            maximum_attempts_per_execution: 1,
            generation_microdollars_per_attempt: 1,
            judging_microdollars_per_attempt: 1,
            tool_microdollars_per_attempt: 0,
            suggestion_calls: 0,
            suggestion_microdollars_per_call: 0,
            auxiliary_calls: 0,
            auxiliary_microdollars_per_call: 0,
        },
    }
}

fn write_native_file(path: &Path, bytes: &[u8], executable: bool) -> Result<()> {
    std::fs::write(path, bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            path,
            std::fs::Permissions::from_mode(if executable { 0o700 } else { 0o600 }),
        )?;
    }
    Ok(())
}
