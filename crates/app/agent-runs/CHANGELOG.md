# Changelog

## [0.66.0] - Unreleased

### Added

- `RepositoryRunLauncher` implements `systemprompt_traits::AgentRunLauncher`; `AgentRunsExtension::launcher(&DbPool) -> DynAgentRunLauncher` is what entry wires into the A2A server. `enqueue` decodes the opaque request as a `RunRequest` and queues a run carrying its task and context; `cancel` records the request and cancels a still-queued run outright; `run_for_task` finds the newest run of a task.
- `update_started` and `finish` run in one transaction with the linked task's transition (`Working`, then the outcome status mapped to a `TaskState`) via `repository::task_link::transition_task`. A task already terminal or missing is logged and left alone, so the run row still records how the run ended. `AgentRunRepository::cancel_queued` and `find_run_for_task` are new. A run reclaimed as failed by `reclaim_stale` does not move its task yet.
- `agent_runs.task_id` and `agent_runs.context_id` (nullable, `idx_agent_runs_task` partial index): the A2A task a run reports through. `NewAgentRun` and `AgentRunRow` carry both as `Option<TaskId>` / `Option<ContextId>`.
- `RunOutcome::final_text` (`#[serde(default)]`): the run's last `assistant_text`, filled by both the Claude Code and Codex CLI outcome classifiers; `RunOutcome::last_assistant_text(events)`.
- `a2a` module: `From<RunStatus> for TaskState` (queued→submitted, preparing/running→working, succeeded→completed, failed/timed_out→failed, cancelled→canceled) and the partial `RunStatus::from_task_state`; `RunOutcome::artifacts(run_id, task_id, context_id)` (one artifact, data part from `structured_output` then text part from `final_text`; `ArtifactMetadata` needs the task and context, so both are parameters); `RunRequest::prompt_from_message` (text verbatim, data as fenced pretty JSON, files as one naming line, blank-line separated); `task_events(run_id, task_id, context_id, event) -> Vec<TaskUpdate>`.
- `TaskUpdate { Status(Box<TaskStatusUpdateEvent>), Artifact(Box<TaskArtifactUpdateEvent>) }`: core keeps the two A2A streaming payloads as separate structs in `systemprompt-agent`, so this crate defines the one enum over both. The crate now depends on `systemprompt-agent`.
- New crate: a host-generic headless coding-agent executor over one `agent_runs` table shared by every origin.
- Contract types: `RunRequest`, `HostKind`, `PermissionMode`, `McpServerSpec`, the `RunEvent` / `RunEventPayload` / `RunEventKind` event stream with `HookDecision` and `RunnerLevel`, `RunStatus`, `ErrorClass`, `RunOutcome` and `RunLayout`.
- Host adapters behind `HeadlessHost` and `host_for(kind) -> Execution`: Claude Code (`claude -p --output-format stream-json`) and Codex CLI (`codex exec --json`). OpenCode and Hermes resolve to `Execution::Unsupported` with a stated reason.
- `execute` / `execute_with_host`: spawns and supervises the host, pumps decoded events into an `EventSink`, enforces the wall clock and cancellation, snapshots the workspace diff, and always deletes `home/`. Host session stores named by `persistent_home_paths` are kept under `host/home-keep/` across attempts.
- `RunCredentialService`: issues and revokes per-run credentials (PAT, gateway session, hook token) so every gateway call is attributed to the run's user.
- `governance::hooks_file`: writes the http-hooks governance plugin for Claude Code; other hosts are governed at the gateway only.
- `AgentRunRepository`: queue insert, `claim_queued` (`FOR UPDATE SKIP LOCKED`), lease renewal, `reclaim_stale` with `resume_session_id` carry-over, event batches, cancel requests and an idempotent `finish`.
- `LeaseRenewer` (30 s renew, 90 s lease) and `ClaimLoop` with `RunHandler`, `ClaimedRun`, `DbEventSink` and `claimer_id`, run from an extension's `RunnerService`.
- `AgentRunsExtension` owns the `agent_runs` and `agent_run_events` tables.
- `RunRequest::attribution_scopes` is a list of `ScopeBinding`s sent header-only: both hosts put `x-systemprompt-scope-<dimension>: <value>` on every gateway call (`RunRequest::scope_headers`), so a workflow or project can roll traces and cost up per run without binding the run's key to the value.
