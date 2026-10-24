# Changelog

## [0.73.0] - Unreleased

### Changed

- Type the remaining raw-string identifier fields.
- Header and gateway path constants spelled once.
- One HostKind with seven ids, app distinct from CLI.

## [0.70.0] - 2026-10-18

### Added

- `AgentRunLauncher::updates_after` maps a run's events past a `seq` to A2A stream frames and reports the run finished once it is terminal and drained.

### Changed

- **Breaking:** `RunCredentialService::new` takes an `ApiKeyLifetime`, built from `api_key_max_lifetime_days`.
- The Claude Code host runs in `RunRequest::workspace`, falling back to the run's scratch workspace when the request names none.

## [0.68.0] - 2026-10-14

### Added

- A run whose credentials carry a purpose also gets an `aud=mcp` JWT for the run's user with that purpose as its signed `purpose` claim (`credentials::mcp_token`), since an instance MCP server authenticates a JWT, not a PAT. Issue order is PAT, gateway session, MCP token, hook token; a failure revokes what was issued, and revoke revokes the MCP token's `jti` with the others.

### Breaking

- `RunCredentials` gains `mcp_token`, `mcp_token_jti` and `mcp_token_expires_at`; struct literals must name them.

## [0.66.0] - 2026-10-13

### Added

- `IssueRunCredentials::purpose`: the caller purpose (`farm:<run_id>`, `workflow:<id>`) bound to the run's PAT at issue time through `IssueApiKeyParams::purpose`, so the connector broker reads it from the credential and never from a client header.

- `runner` module: `AgentRunsExtension::runner_services()` returns `A2aRunnerService`, which runs a `ClaimLoop` over `AGENT_RUN_ORIGIN_KIND` (`"a2a"`, re-exported from the agent domain) with at most `A2A_RUNNER_MAX_CONCURRENT` (2) runs. Its `A2aRunHandler` decodes the stored `RunRequest`, issues credentials to the row's user (`a2a-run-<id>`, no scopes, the request's plugin), creates the run directory under `<storage data>/agent-runs/<id>`, writes the hooks file when the request names a plugin, executes, finishes the row (which moves the task) and revokes the credentials; a failure before the outcome finishes the row `failed/host_crashed`, and a recorded cancel request cancels the run. `A2aRunnerConfig::new(api_internal_url, jwt_issuer, data_root)` derives the gateway base and hook URLs from `Config::api_internal_url`. The crate now depends on `systemprompt-manifest`, `systemprompt-config` and `systemprompt-provider-contracts`.
- `reclaim_stale` runs in one transaction and moves the A2A task of every run it fails on the final attempt (`host_crashed`) to `Failed` through `task_link::transition_task`, so a crashed runner no longer leaves the task `Working`.
- `RepositoryRunLauncher` implements `systemprompt_traits::AgentRunLauncher`; `AgentRunsExtension::launcher(&DbPool) -> DynAgentRunLauncher` is what entry wires into the A2A server. `enqueue` decodes the opaque request as a `RunRequest` and queues a run carrying its task and context; `cancel` records the request and cancels a still-queued run outright; `run_for_task` finds the newest run of a task.
- `update_started` and `finish` run in one transaction with the linked task's transition (`Working`, then the outcome status mapped to a `TaskState`) via `repository::task_link::transition_task`. A task already terminal or missing is logged and left alone, so the run row still records how the run ended. `AgentRunRepository::cancel_queued` and `find_run_for_task` are new.
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
