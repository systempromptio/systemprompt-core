//! Durable evaluator supervisor composed from fenced domain repositories.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};
use systemprompt_database::DbPool;
use systemprompt_evaluation::experiments::execution::{ArtifactEvidence, ArtifactFile, ClientCapabilities, EvidenceArchive, ExecutionEvidence, ExecutionLimits};
use systemprompt_evaluation::experiments::resources::{CaseContent, ResourceContent, RubricContent};
use systemprompt_evaluation::experiments::scoring::{self, EvidenceJudgment};
use systemprompt_evaluation::experiments::verification::{self, VerificationInput};
use systemprompt_evaluation::repository::experiments::{AssignmentRepository, DeterministicMeasurement, EvidenceRepository, EvaluationLifecycleRepository, EvaluationTrafficClass, ExecutionCapabilityRepository, ExecutionCompletion, ExecutionEvent, ExecutionEventRepository, ExecutionLease, ExecutionStage, ExperimentRepository, GatewayEvaluationRepository, GeneratedSuggestion, TerminalOutcome, WorkerRepository};
use systemprompt_identifiers::{EvalExecutionId, EvalWorkerId, UserId};
use systemprompt_marketplace::managed::RevisionBundle;

use super::client::{ClientPurpose, NativeClient};
use super::container::{ContainerLaunch, ExecutionNetwork};
use crate::{SchedulerError, SchedulerResult};

#[derive(Debug, Clone)]
pub struct EvaluatorSupervisorConfig {
    pub docker: PathBuf,
    pub workspace_root: PathBuf,
    pub environment: String,
    pub client_image: String,
    pub relay_image: String,
    pub relay_control_network: String,
    pub relay_upstream: String,
}

#[derive(Debug)]
pub struct EvaluatorSupervisor {
    pub(crate) config: EvaluatorSupervisorConfig,
    assignments: AssignmentRepository,
    capabilities: ExecutionCapabilityRepository,
    evidence: EvidenceRepository,
    events: ExecutionEventRepository,
    experiments: ExperimentRepository,
    lifecycle: EvaluationLifecycleRepository,
    gateway: GatewayEvaluationRepository,
    workers: WorkerRepository,
}

impl EvaluatorSupervisor {
    pub fn new(pool: &DbPool, config: EvaluatorSupervisorConfig) -> SchedulerResult<Self> {
        validate_config(&config)?;
        let pool = pool
            .write_pool()
            .ok_or_else(|| SchedulerError::missing_context("writable evaluator database"))?
            .as_ref()
            .clone();
        Ok(Self { config, assignments: AssignmentRepository::new(pool.clone()), capabilities: ExecutionCapabilityRepository::new(pool.clone()), evidence: EvidenceRepository::new(pool.clone()), events: ExecutionEventRepository::new(pool.clone()), experiments: ExperimentRepository::new(pool.clone()), lifecycle: EvaluationLifecycleRepository::new(pool.clone()), gateway: GatewayEvaluationRepository::new(pool.clone()), workers: WorkerRepository::new(pool) })
    }

    pub async fn run_once(&self, owner: &UserId, worker_id: &EvalWorkerId) -> SchedulerResult<bool> {
        let worker = self.workers.get_owned(owner, worker_id, &self.config.environment).await.map_err(internal)?;
        self.lifecycle.reconcile_restart(owner).await.map_err(internal)?;
        self.reconcile_owned_docker(owner).await?;
        let Some(record) = self.experiments.claim(owner, worker_id).await.map_err(internal)? else { return Ok(false); };
        let lease = ExecutionLease::builder(record.id.clone(), worker_id.clone()).fencing_token(record.fencing_token).build().map_err(internal)?;
        let assignment = self.assignments.get(&worker, &lease).await.map_err(internal)?;
        let access = self.capabilities.issue(owner, &lease).await.map_err(internal)?;
        let suffix = safe_suffix(&record.id);
        let directory = self.config.workspace_root.join(&suffix);
        std::fs::create_dir(&directory)?;
        let _workspace_guard = WorkspaceDirectory(directory.clone());
        let home = directory.join("home");
        std::fs::create_dir(&home)?;
        materialize_skills(&assignment.skill_bundle.manifest, &home.join(".claude/skills"))?;
        let installed_skill_state = workspace_state(&home.join(".claude/skills"))?;
        materialize_root(&assignment.configuration.manifest, &home.join("work"))?;
        let mcp = serde_json::json!({"mcpServers":{"evaluation_fixture":{"type":"http","url":format!("http://eval-relay-{suffix}:8090/mcp/evaluation_fixture"),"headers":{"Authorization":format!("Bearer {}", access.expose_token()),"x-session-id":access.session_id.as_str()}}}});
        write_private(&home.join(".mcp.json"), &serde_json::to_vec(&mcp).map_err(internal)?)?;
        write_private(&directory.join("client.env"), format!("ANTHROPIC_BASE_URL=http://eval-relay-{suffix}:8090\nANTHROPIC_AUTH_TOKEN={}\nANTHROPIC_CUSTOM_HEADERS=x-session-id: {}\nSYSTEMPROMPT_EXECUTION_ID={}\n", access.expose_token(), access.session_id, record.id).as_bytes())?;
        self.append_event(&worker, &lease, 0, ExecutionStage::Provisioning, "Installed exact managed assignment bundles").await?;
        let network_name = format!("eval-net-{suffix}");
        let relay_name = format!("eval-relay-{suffix}");
        let client_name = format!("eval-client-{suffix}");
        let mut network = ExecutionNetwork::create(self.config.docker.clone(), network_name.clone(), owner.as_str(), record.id.as_str())?;
        network.start_relay(&self.config.relay_image, &self.config.relay_control_network, &self.config.relay_upstream, relay_name.clone())?;
        let variant = assignment.spec.variants.get(usize::try_from(record.variant_index).map_err(|error| SchedulerError::Internal(error.to_string()))?).ok_or_else(|| SchedulerError::config_error("Assignment variant is unavailable"))?;
        let client = NativeClient::builder(variant.client, variant.model.clone()).limits(ExecutionLimits::default()).build().map_err(internal)?;
        let launch = ContainerLaunch::builder(self.config.docker.clone(), directory.clone()).image(self.config.client_image.clone()).network(network.name().to_owned()).name(client_name.clone()).ownership(owner.as_str(), record.id.as_str()).build()?;
        let case = match assignment.case { ResourceContent::Case(case) => case, _ => return Err(SchedulerError::config_error("Assignment case type changed")) };
        let rubric = match assignment.rubric { ResourceContent::Rubric(rubric) => rubric, _ => return Err(SchedulerError::config_error("Assignment rubric type changed")) };
        install_case_fixtures(&case, &home.join("work"))?;
        let baseline = workspace_state(&home.join("work"))?;
        self.gateway.set_traffic_class(owner, &lease, match assignment.spec.execution_mode { systemprompt_evaluation::experiments::ExecutionMode::Fixture => EvaluationTrafficClass::Fixture, systemprompt_evaluation::experiments::ExecutionMode::Live => EvaluationTrafficClass::LiveEvaluation }).await.map_err(internal)?;
        let prompt = execution_prompt(&case)?;
        let mut execution = launch.start(&client, &prompt)?;
        network.verify(&[client_name.clone(), relay_name.clone()])?;
        self.append_event(&worker, &lease, 1, ExecutionStage::Context, "Started isolated Claude Code execution and authenticated relay").await?;
        let started = Instant::now(); let mut last_heartbeat = Instant::now();
        let status = loop {
            if let Some(status) = execution.poll()? { break status; }
            if last_heartbeat.elapsed() >= Duration::from_secs(20) {
                if let Err(error) = self.experiments.heartbeat(owner, &lease).await {
                    let cancel = execution.cancel();
                    let isolated = network.cleanup();
                    let cleaned = std::fs::remove_dir_all(&directory);
                    let confirmed = cancel.is_ok() && isolated.is_ok() && cleaned.is_ok();
                    self.lifecycle.record_cleanup(owner, &lease, Some(&client_name), Some(&network_name), confirmed, (!confirmed).then_some("Cancellation cleanup was not fully acknowledged")).await.map_err(internal)?;
                    return Err(internal(error));
                }
                last_heartbeat = Instant::now();
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        };
        self.append_event(&worker, &lease, 2, ExecutionStage::Verification, "Client exited; collecting deterministic evidence").await?;
        let (stdout_path, stderr_path) = execution.output_paths();
        let stdout = std::fs::read(stdout_path)?; let stderr = std::fs::read(stderr_path)?;
        let mut artifacts = BTreeMap::new();
        artifacts.insert("client-events.jsonl".to_owned(), ArtifactFile { bytes: stdout.clone(), executable: false });
        artifacts.insert("client-stderr.log".to_owned(), ArtifactFile { bytes: stderr.clone(), executable: false });
        artifacts.extend(changed_workspace(&home.join("work"), &baseline)?);
        let observed_skill_state = workspace_state(&home.join(".claude/skills"))?;
        let installation_matches = installed_skill_state == observed_skill_state;
        artifacts.insert("installation-integrity.json".to_owned(), ArtifactFile {
            bytes: serde_jcs::to_vec(&serde_json::json!({
                "expected": &installed_skill_state,
                "observed": &observed_skill_state,
                "matches": installation_matches,
            })).map_err(internal)?,
            executable: false,
        });
        let judgment = if status.success() {
            let evidence_dir = home.join("work/evidence");
            std::fs::create_dir_all(&evidence_dir)?;
            write_private(&evidence_dir.join("client-events.jsonl"), &stdout)?;
            self.gateway.set_traffic_class(owner, &lease, EvaluationTrafficClass::Judge).await.map_err(internal)?;
            let judge_name = format!("eval-judge-{suffix}");
            let judge_launch = ContainerLaunch::builder(self.config.docker.clone(), directory.clone()).image(self.config.client_image.clone()).network(network.name().to_owned()).name(judge_name.clone()).output_stem("judge").ownership(owner.as_str(), record.id.as_str()).build()?;
            let judge_prompt = judgment_prompt(&case, &rubric, artifacts.keys())?;
            let mut judge = judge_launch.start_for(&client, ClientPurpose::Judge, &judge_prompt)?;
            network.verify(&[judge_name, relay_name.clone()])?;
            let judge_status = loop {
                if let Some(status) = judge.poll()? { break status; }
                if last_heartbeat.elapsed() >= Duration::from_secs(20) {
                    if let Err(error) = self.experiments.heartbeat(owner, &lease).await {
                        let cancel = judge.cancel();
                        let isolated = network.cleanup();
                        let cleaned = std::fs::remove_dir_all(&directory);
                        let confirmed = cancel.is_ok() && isolated.is_ok() && cleaned.is_ok();
                        self.lifecycle.record_cleanup(owner, &lease, Some(&client_name), Some(&network_name), confirmed, (!confirmed).then_some("Cancellation cleanup was not fully acknowledged")).await.map_err(internal)?;
                        return Err(internal(error));
                    }
                    last_heartbeat = Instant::now();
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            };
            let (judge_stdout, judge_stderr) = judge.output_paths();
            let judge_bytes = std::fs::read(judge_stdout)?;
            artifacts.insert("judge-events.jsonl".to_owned(), ArtifactFile { bytes: judge_bytes.clone(), executable: false });
            artifacts.insert("judge-stderr.log".to_owned(), ArtifactFile { bytes: std::fs::read(judge_stderr)?, executable: false });
            self.append_event(&worker, &lease, 3, ExecutionStage::Verification, "Completed separately metered bounded semantic judgment").await?;
            if judge_status.success() { parse_judgment(&judge_bytes).ok() } else { None }
        } else { None };
        let retained_requests = self.evidence.list_request_ids(owner, &record.id).await.map_err(internal)?;
        let preliminary_references = artifacts.keys().cloned().chain(retained_requests.iter().map(|request| request.as_str().to_owned())).collect::<BTreeSet<_>>();
        let preliminary_archive = EvidenceArchive { files: artifacts.clone() };
        let deterministic = verification::evaluate(VerificationInput { case: &case, evidence: &preliminary_archive });
        let semantic_passed = judgment.as_ref().and_then(|value| scoring::score(&rubric, value, &preliminary_references).ok()).is_some_and(|outcome| outcome.passed);
        let should_suggest = status.success()
            && (!deterministic.hard_failures.is_empty() || deterministic.checks.values().any(|passed| !passed) || !semantic_passed)
            && assignment.spec.frozen.as_ref().is_some_and(|frozen| frozen.cost_envelope.suggestion_calls > 0)
            && self.lifecycle.should_generate_suggestion(owner, &record.id, assignment.spec.frozen.as_ref().map_or(0, |frozen| frozen.cost_envelope.suggestion_calls)).await.map_err(internal)?;
        if should_suggest {
            self.gateway.set_traffic_class(owner, &lease, EvaluationTrafficClass::Suggestion).await.map_err(internal)?;
            let suggestion_name = format!("eval-suggestion-{suffix}");
            let suggestion_launch = ContainerLaunch::builder(self.config.docker.clone(), directory.clone()).image(self.config.client_image.clone()).network(network.name().to_owned()).name(suggestion_name.clone()).output_stem("suggestion").ownership(owner.as_str(), record.id.as_str()).build()?;
            let suggestion_prompt = suggestion_prompt(&case, &deterministic.hard_failures, artifacts.keys())?;
            let mut suggestion_run = suggestion_launch.start_for(&client, ClientPurpose::Suggestion, &suggestion_prompt)?;
            network.verify(&[suggestion_name, relay_name.clone()])?;
            let suggestion_status = loop {
                if let Some(status) = suggestion_run.poll()? { break status; }
                if last_heartbeat.elapsed() >= Duration::from_secs(20) {
                    if let Err(error) = self.experiments.heartbeat(owner, &lease).await {
                        let cancel = suggestion_run.cancel();
                        let isolated = network.cleanup();
                        let cleaned = std::fs::remove_dir_all(&directory);
                        let confirmed = cancel.is_ok() && isolated.is_ok() && cleaned.is_ok();
                        self.lifecycle.record_cleanup(owner, &lease, Some(&client_name), Some(&network_name), confirmed, (!confirmed).then_some("Suggestion cancellation cleanup was not fully acknowledged")).await.map_err(internal)?;
                        return Err(internal(error));
                    }
                    last_heartbeat = Instant::now();
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            };
            let (suggestion_stdout, suggestion_stderr) = suggestion_run.output_paths();
            let suggestion_bytes = std::fs::read(suggestion_stdout)?;
            artifacts.insert("suggestion-events.jsonl".to_owned(), ArtifactFile { bytes: suggestion_bytes.clone(), executable: false });
            artifacts.insert("suggestion-stderr.log".to_owned(), ArtifactFile { bytes: std::fs::read(suggestion_stderr)?, executable: false });
            if suggestion_status.success() {
                if let Ok(generated) = parse_suggestion(&suggestion_bytes) {
                    let after = self.evidence.list_request_ids(owner, &record.id).await.map_err(internal)?;
                    if let Some(request) = after.iter().find(|request| !retained_requests.contains(request)) {
                        self.lifecycle.record_generated_suggestion(owner, &record.id, request, &generated).await.map_err(internal)?;
                    }
                }
            }
            self.append_event(&worker, &lease, 4, ExecutionStage::Verification, "Generated a separately metered development-only suggestion").await?;
        }
        let artifact_evidence = artifacts.iter().map(|(relative_path, file)| ArtifactEvidence { relative_path: relative_path.clone(), sha256: hex::encode(Sha256::digest(&file.bytes)), bytes: file.bytes.len() as u64 }).collect();
        let network_cleanup = network.cleanup();
        let cleanup = std::fs::remove_dir_all(&directory);
        let cleanup_confirmed = cleanup.is_ok() && network_cleanup.is_ok();
        let cleanup_error = cleanup.err().map(|error| error.to_string())
            .or_else(|| network_cleanup.err().map(|error| error.to_string()));
        self.lifecycle.record_cleanup(owner, &lease, Some(&client_name), Some(&network_name), cleanup_confirmed, cleanup_error.as_deref()).await.map_err(internal)?;
        let requests = self.evidence.list_request_ids(owner, &record.id).await.map_err(internal)?;
        let evidence = ExecutionEvidence::builder().execution_id(record.id.clone()).fencing_token(record.fencing_token).capabilities(ClientCapabilities { client: variant.client, client_version: variant.client_version.clone(), adapter_version: "rust-evaluator-supervisor-v1".to_owned(), image_digest: image_digest(&self.config.client_image)?, supports_session_resume: false }).installed_bundle_digest(assignment.skill_bundle.digest.clone()).candidate_bundle_digest(variant.skill_bundle_digest.clone()).workspace_digest(assignment.configuration.digest.clone()).requests(requests).artifacts(artifact_evidence).exit_code(status.code()).elapsed_milliseconds(started.elapsed().as_millis() as u64).cleanup_confirmed(cleanup_confirmed).build().map_err(internal)?;
        self.evidence.submit(owner, &lease, &evidence, &EvidenceArchive { files: artifacts }).await.map_err(internal)?;
        let outcome = if status.success() && cleanup_confirmed { TerminalOutcome::Completed } else { TerminalOutcome::Error };
        self.experiments.complete(owner, &lease, &ExecutionCompletion { outcome, summary: if outcome == TerminalOutcome::Completed { "Execution, evidence export and cleanup verified" } else { "Execution failed or cleanup remains pending" }.to_owned() }).await.map_err(internal)?;
        if outcome == TerminalOutcome::Completed {
            let archive = self.evidence.get_artifacts(owner, &record.id).await.map_err(internal)?;
            let deterministic = verification::evaluate(VerificationInput { case: &case, evidence: &archive });
            let references = evidence_references(&evidence);
            let scored = judgment.as_ref().and_then(|value| scoring::score(&rubric, value, &references).ok().map(|outcome| (value.clone(), outcome)));
            let accounting = self.lifecycle.execution_accounting(owner, &record.id).await.map_err(internal)?;
            let deterministic_passed = deterministic.hard_failures.is_empty() && deterministic.checks.values().all(|value| *value);
            let measurement = DeterministicMeasurement {
                hard_failures: deterministic.hard_failures,
                checks: deterministic.checks,
                judgment: scored.as_ref().map(|(judgment, _)| judgment.clone()),
                quality_milli: scored.as_ref().map(|(_, outcome)| outcome.score_milli),
                latency_ms: started.elapsed().as_millis() as u64,
                input_tokens: accounting.input_tokens,
                output_tokens: accounting.output_tokens,
                tool_calls: accounting.tool_calls,
                attempted_cost_microdollars: accounting.attempted_cost_microdollars,
                accounting_status: accounting.status,
                verified_success: deterministic_passed && scored.as_ref().is_some_and(|(_, outcome)| outcome.passed),
            };
            self.lifecycle.record_measurement(owner, &lease, &measurement).await.map_err(internal)?;
        }
        Ok(true)
    }

    async fn append_event(&self, worker: &systemprompt_evaluation::repository::experiments::WorkerRecord, lease: &ExecutionLease, sequence: i64, stage: ExecutionStage, summary: &str) -> SchedulerResult<()> {
        let event = ExecutionEvent::builder(sequence, stage).summary(summary.to_owned()).build().map_err(internal)?;
        self.events.append(worker, lease, &event).await.map_err(internal)
    }

    async fn reconcile_owned_docker(&self, owner: &UserId) -> SchedulerResult<()> {
        for kind in ["container", "network"] {
            let owner_filter = format!("label=systemprompt.evaluator.owner={}", owner.as_str());
            let list_args = if kind == "container" { vec!["ps", "-aq", "--filter", owner_filter.as_str()] } else { vec!["network", "ls", "-q", "--filter", owner_filter.as_str()] };
            let output = std::process::Command::new(&self.config.docker).args(&list_args).output()?;
            if !output.status.success() { return Err(SchedulerError::config_error("Unable to enumerate owned evaluator Docker objects")); }
            for id in String::from_utf8(output.stdout).map_err(internal)?.lines().filter(|id| !id.is_empty()) {
                let format = if kind == "container" { "{{index .Config.Labels \"systemprompt.evaluator.execution\"}}" } else { "{{index .Labels \"systemprompt.evaluator.execution\"}}" };
                let inspect_args = if kind == "container" { vec!["inspect", "--format", format, id] } else { vec!["network", "inspect", "--format", format, id] };
                let inspected = std::process::Command::new(&self.config.docker).args(inspect_args).output()?;
                if !inspected.status.success() { return Err(SchedulerError::config_error("Unable to inspect owned evaluator Docker object")); }
                let execution = String::from_utf8(inspected.stdout).map_err(internal)?.trim().to_owned();
                if execution.is_empty() || !self.lifecycle.execution_is_live(owner, &EvalExecutionId::new(execution)).await.map_err(internal)? {
                    let remove_args = if kind == "container" { vec!["rm", "--force", id] } else { vec!["network", "rm", id] };
                    let removed = std::process::Command::new(&self.config.docker).args(remove_args).output()?;
                    if !removed.status.success() { return Err(SchedulerError::config_error("Owned evaluator Docker cleanup was not acknowledged")); }
                }
            }
        }
        Ok(())
    }
}

fn execution_prompt(case: &CaseContent) -> SchedulerResult<String> {
    Ok(format!(
        "Execute this immutable evaluation case. Use only the installed skills and configured evaluation fixture MCP service. Do not use a shell or network. Make any requested platform test-record write at most once, read it back, and restore it. Your final response must answer the case and cite the fixture/tool evidence used.\n\nCASE PROMPT:\n{}\n\nEXPECTED BEHAVIOURS (do not merely repeat these):\n{}\n\nNAMED DETERMINISTIC ASSERTIONS:\n{}",
        case.prompt,
        serde_json::to_string(&case.expected_behavior).map_err(internal)?,
        serde_json::to_string(&case.assertions).map_err(internal)?,
    ))
}

fn judgment_prompt<'a>(
    case: &CaseContent,
    rubric: &RubricContent,
    evidence: impl Iterator<Item = &'a String>,
) -> SchedulerResult<String> {
    let evidence = evidence.cloned().collect::<Vec<_>>();
    Ok(format!(
        "Judge the completed evaluation using only the retained files listed below. Read evidence/client-events.jsonl when needed. Return only one JSON object matching {{\"dimensions\":[{{\"name\":string,\"score\":integer 1..5,\"evidence\":[exact retained reference]}}],\"hard_gates\":{{string:boolean}},\"rationale\":string}}. Include every rubric dimension and exactly every hard gate. Never invent a reference. A missing or ambiguous fact must reduce the score.\n\nCASE:\n{}\n\nEXPECTED:\n{}\n\nRUBRIC:\n{}\n\nRETAINED REFERENCES:\n{}",
        case.prompt,
        serde_json::to_string(&case.expected_behavior).map_err(internal)?,
        serde_json::to_string(rubric).map_err(internal)?,
        serde_json::to_string(&evidence).map_err(internal)?,
    ))
}

fn parse_judgment(bytes: &[u8]) -> SchedulerResult<EvidenceJudgment> {
    let body = std::str::from_utf8(bytes).map_err(internal)?;
    for line in body.lines().rev() {
        let Ok(event) = serde_json::from_str::<serde_json::Value>(line) else { continue; };
        let candidate = event.get("result").and_then(serde_json::Value::as_str)
            .or_else(|| event.pointer("/message/content/0/text").and_then(serde_json::Value::as_str));
        let Some(candidate) = candidate else { continue; };
        if let Ok(judgment) = serde_json::from_str(candidate) { return Ok(judgment); }
        if let (Some(start), Some(end)) = (candidate.find('{'), candidate.rfind('}')) {
            if start <= end {
                if let Ok(judgment) = serde_json::from_str(&candidate[start..=end]) { return Ok(judgment); }
            }
        }
    }
    Err(SchedulerError::config_error("Semantic judge returned no valid evidence judgment"))
}

fn parse_suggestion(bytes: &[u8]) -> SchedulerResult<GeneratedSuggestion> {
    parse_client_json(bytes, "suggestion")
}

fn parse_client_json<T: serde::de::DeserializeOwned>(bytes: &[u8], label: &str) -> SchedulerResult<T> {
    let body = std::str::from_utf8(bytes).map_err(internal)?;
    for line in body.lines().rev() {
        let Ok(event) = serde_json::from_str::<serde_json::Value>(line) else { continue; };
        let candidate = event.get("result").and_then(serde_json::Value::as_str)
            .or_else(|| event.pointer("/message/content/0/text").and_then(serde_json::Value::as_str));
        let Some(candidate) = candidate else { continue; };
        if let Ok(value) = serde_json::from_str(candidate) { return Ok(value); }
        if let (Some(start), Some(end)) = (candidate.find('{'), candidate.rfind('}')) {
            if start <= end {
                if let Ok(value) = serde_json::from_str(&candidate[start..=end]) { return Ok(value); }
            }
        }
    }
    Err(SchedulerError::config_error(format!("Client returned no valid {label} JSON")))
}

fn suggestion_prompt<'a>(case: &CaseContent, failures: &[String], evidence: impl Iterator<Item = &'a String>) -> SchedulerResult<String> {
    Ok(format!(
        "Using only the retained development-case evidence, propose a candidate skill change. Never use or reveal holdout content. Return only one JSON object matching {{\"proposed_changes\":object,\"hypothesis\":string,\"supporting_failures\":[string],\"originating_evidence\":[exact retained reference]}}.\n\nCASE:\n{}\n\nFAILURES:\n{}\n\nEVIDENCE:\n{}",
        case.prompt,
        serde_json::to_string(failures).map_err(internal)?,
        serde_json::to_string(&evidence.cloned().collect::<Vec<_>>()).map_err(internal)?,
    ))
}

fn install_case_fixtures(case: &CaseContent, root: &Path) -> SchedulerResult<()> {
    for (relative, content) in &case.fixtures {
        let path = root.join(relative);
        if let Some(parent) = path.parent() { std::fs::create_dir_all(parent)?; }
        write_private(&path, content.as_bytes())?;
    }
    Ok(())
}

fn workspace_state(root: &Path) -> SchedulerResult<BTreeMap<String, String>> {
    let mut files = BTreeMap::new();
    visit_workspace(root, root, &mut |relative, bytes, _| {
        files.insert(relative, hex::encode(Sha256::digest(bytes)));
        Ok(())
    })?;
    Ok(files)
}

fn changed_workspace(root: &Path, baseline: &BTreeMap<String, String>) -> SchedulerResult<BTreeMap<String, ArtifactFile>> {
    let mut files = BTreeMap::new();
    let mut bytes_total = 0usize;
    visit_workspace(root, root, &mut |relative, bytes, executable| {
        let digest = hex::encode(Sha256::digest(bytes));
        if baseline.get(&relative) != Some(&digest) {
            bytes_total = bytes_total.checked_add(bytes.len()).ok_or_else(|| SchedulerError::config_error("Workspace evidence size overflow"))?;
            if files.len() >= 252 || bytes_total > 15 * 1024 * 1024 { return Err(SchedulerError::config_error("Workspace evidence exceeds retained limits")); }
            files.insert(format!("workspace/{relative}"), ArtifactFile { bytes: bytes.to_vec(), executable });
        }
        Ok(())
    })?;
    Ok(files)
}

fn visit_workspace(
    root: &Path,
    directory: &Path,
    visitor: &mut impl FnMut(String, &[u8], bool) -> SchedulerResult<()>,
) -> SchedulerResult<()> {
    let mut entries = std::fs::read_dir(directory)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        let metadata = std::fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() { return Err(SchedulerError::config_error("Evaluation workspace contains a link")); }
        if metadata.is_dir() { visit_workspace(root, &path, visitor)?; }
        else if metadata.is_file() {
            let relative = path.strip_prefix(root).map_err(internal)?.to_string_lossy().replace('\\', "/");
            #[cfg(unix)]
            let executable = {
                use std::os::unix::fs::PermissionsExt;
                metadata.permissions().mode() & 0o111 != 0
            };
            #[cfg(not(unix))]
            let executable = false;
            visitor(relative, &std::fs::read(path)?, executable)?;
        } else { return Err(SchedulerError::config_error("Evaluation workspace contains a non-regular file")); }
    }
    Ok(())
}

fn evidence_references(evidence: &ExecutionEvidence) -> BTreeSet<String> {
    let mut references = BTreeSet::new();
    for artifact in &evidence.artifacts {
        references.insert(artifact.relative_path.clone());
        references.insert(artifact.sha256.clone());
    }
    for request in &evidence.requests { references.insert(request.as_str().to_owned()); }
    references
}

fn decoded_bundle(value: &serde_json::Value) -> SchedulerResult<RevisionBundle> {
    let bundle: RevisionBundle = serde_json::from_value(value.clone()).map_err(|error| SchedulerError::Internal(error.to_string()))?;
    bundle.verify().map_err(internal)?;
    Ok(bundle)
}

fn materialize_root(value: &serde_json::Value, destination: &Path) -> SchedulerResult<()> {
    let bundle = decoded_bundle(value)?;
    install_files(&bundle.revision_files(&bundle.root).map_err(internal)?.0, destination)
}

fn materialize_skills(value: &serde_json::Value, destination: &Path) -> SchedulerResult<()> {
    let bundle = decoded_bundle(value)?;
    for revision in bundle.revisions.keys() {
        let files = bundle.revision_files(revision).map_err(internal)?;
        let config = files.0.get("config.yaml").and_then(|file| serde_yaml::from_slice::<systemprompt_models::DiskSkillConfig>(&file.bytes).ok());
        if let Some(config) = config {
            let id = if config.id.as_str().is_empty() { revision.as_str() } else { config.id.as_str() };
            let directory = destination.join(id.replace('_', "-"));
            std::fs::create_dir_all(&directory)?;
            let content = files.0.get(config.content_file()).ok_or_else(|| SchedulerError::config_error("Managed skill content is missing"))?;
            let body = std::str::from_utf8(&content.bytes).map_err(|error| SchedulerError::Internal(error.to_string()))?;
            let skill_md = format!("---\nname: {}\ndescription: {:?}\n---\n\n{}", id.replace('_', "-"), config.description, systemprompt_models::strip_frontmatter(body));
            write_private(&directory.join("SKILL.md"), skill_md.as_bytes())?;
            for (path, file) in files.0.iter().filter(|(path, _)| path.as_str() != "config.yaml" && path.as_str() != config.content_file()) {
                install_file(&directory.join(path), file)?;
            }
        } else {
            install_files(&files.0, &destination.join(revision.as_str()))?;
        }
    }
    Ok(())
}

fn install_files(files: &BTreeMap<String, systemprompt_marketplace::managed::AssetFile>, destination: &Path) -> SchedulerResult<()> {
    for (path, file) in files { install_file(&destination.join(path), file)?; }
    Ok(())
}

fn install_file(path: &Path, file: &systemprompt_marketplace::managed::AssetFile) -> SchedulerResult<()> {
    if let Some(parent) = path.parent() { std::fs::create_dir_all(parent)?; }
    write_private(path, &file.bytes)?;
    #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; std::fs::set_permissions(path, std::fs::Permissions::from_mode(if file.executable { 0o700 } else { 0o600 }))?; }
    Ok(())
}

fn write_private(path: &Path, bytes: &[u8]) -> SchedulerResult<()> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new(); options.create_new(true).write(true);
    #[cfg(unix)] { use std::os::unix::fs::OpenOptionsExt; options.mode(0o600); }
    options.open(path)?.write_all(bytes)?; Ok(())
}

fn validate_config(config: &EvaluatorSupervisorConfig) -> SchedulerResult<()> {
    if !config.docker.is_absolute() || !config.workspace_root.is_absolute() || config.environment.trim().is_empty() || !config.client_image.contains("@sha256:") || !config.relay_image.contains("@sha256:") || matches!(config.relay_control_network.as_str(), "host" | "bridge" | "default" | "none") || !matches!(config.relay_upstream.as_str(), value if value.starts_with("http://") || value.starts_with("https://")) { return Err(SchedulerError::config_error("Evaluator supervisor requires absolute paths, pinned images, dedicated relay network and HTTP(S) upstream")); }
    Ok(())
}

fn safe_suffix(execution: &EvalExecutionId) -> String { execution.as_str().bytes().filter(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')).map(char::from).take(48).collect() }
fn image_digest(image: &str) -> SchedulerResult<String> { image.rsplit_once("@sha256:").map(|(_, digest)| digest.to_owned()).ok_or_else(|| SchedulerError::config_error("Pinned image digest missing")) }
fn internal(error: impl std::fmt::Display) -> SchedulerError { SchedulerError::Internal(error.to_string()) }

#[derive(Debug)]
struct WorkspaceDirectory(PathBuf);

impl Drop for WorkspaceDirectory {
    fn drop(&mut self) { drop(std::fs::remove_dir_all(&self.0)); }
}
