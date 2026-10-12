# systemprompt-headless

[![Crates.io](https://img.shields.io/crates/v/systemprompt-headless.svg?style=flat-square)](https://crates.io/crates/systemprompt-headless)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-headless?style=flat-square)](https://docs.rs/systemprompt-headless)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

A host-generic headless coding-agent executor. It runs a coding-agent CLI as an owned, supervised subprocess, decodes its output into one `RunEvent` contract, issues each run ephemeral attributed credentials, and drives a leased claim queue over a single `headless_runs` table shared by every product that queues runs.

**Layer**: App. It composes the users and oauth domain crates for credential issuance. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Hosts

| `HostKind` | Spelling | Status | Invocation / reason |
|------------|----------|--------|---------------------|
| `ClaudeCode` | `claude-code` | supported | `claude -p --output-format stream-json`; hooks, resume, structured output, MCP |
| `CodexCli` | `codex-cli` | supported | `codex exec --json`; resume, structured output, MCP; no hook surface |
| `OpenCode` | `opencode` | unsupported | OpenCode 0.5.1 `run` prints no machine-readable event stream and `serve` is an HTTP server, so there is no line protocol to supervise |
| `Hermes` | `hermes` | unsupported | not installed on any verified host and its headless output has not been captured, so no decoder exists |

`host_for(kind)` returns `Execution::Supported(Box<dyn HeadlessHost>)` or `Execution::Unsupported { reason }`. `execute` on an unsupported host returns `HeadlessError::UnsupportedHost` after cleaning `home/`. Each adapter reports its `HostCapabilities { hooks, resume, structured_output, mcp }`.

## Contract

### `RunRequest`

Stored as `headless_runs.request` (JSONB). The `host` column is taken from `request.host`.

| Field | Type | Meaning |
|-------|------|---------|
| `host` | `HostKind` | which adapter runs it |
| `prompt` | `String` | the task |
| `system_prompt_append` | `Option<String>` | appended to the host's system prompt |
| `persona` | `Option<String>` | persona text (Codex writes it as the workspace `AGENTS.md`) |
| `model` | `Option<String>` | model override |
| `allowed_tools` / `disallowed_tools` | `Vec<String>` | tool allow / deny lists |
| `permission_mode` | `PermissionMode` | the host's permission mode |
| `permission_prompts_none` | `bool` | never prompt for permission |
| `workspace` | `PathBuf` | the working tree the host edits |
| `add_dirs` | `Vec<PathBuf>` | extra readable directories |
| `mcp_servers` | `BTreeMap<String, McpServerSpec>` | MCP servers handed to the host |
| `output_schema` | `Option<Value>` | JSON schema for a structured final answer |
| `max_turns` | `Option<u32>` | turn ceiling |
| `max_budget_usd` | `Option<f64>` | spend ceiling |
| `wall_clock` | `Duration` | kill deadline |
| `resume_session_id` | `Option<HostSessionId>` | host session to resume; set by stale reclaim |
| `attempt` | `u32` | attempt number; set by stale reclaim |
| `attribution_scopes` | `Vec<String>` | attribution scopes for the run's credentials |
| `plugin_id` | `Option<PluginId>` | plugin the run is attributed to; keys the hooks URLs |
| `plugin_dirs` | `Vec<PathBuf>` | extra plugin directories for the host |

### Event kinds

`RunEventPayload` is serialised with `kind` as its tag. `RunEvent` adds `seq`, `kind`, `tool_name`, `tool_use_id` and `created_at`; rows land in `headless_run_events` keyed `(run_id, seq)` in arrival order.

| `kind` | Payload fields | Source |
|--------|----------------|--------|
| `init` | `session_id`, `model`, `tools`, `cwd` | host start |
| `assistant_text` | `text` | assistant message |
| `tool_use` | `tool_use_id`, `name`, `input` | tool call |
| `tool_result` | `tool_use_id`, `content`, `is_error` | tool result |
| `hook` | `name`, `decision`, `policy` | governance hook verdict |
| `result` | `subtype`, `is_error`, `num_turns`, `duration_ms`, `input_tokens`, `output_tokens`, `cost_microdollars`, `structured_output`, `error` | host's final record |
| `stderr` | `line` | tail of the host's stderr |
| `runner` | `message`, `level` (`info` / `warn` / `error`) | written by the executor itself |

### `HookDecision`

| Value | Meaning |
|-------|---------|
| `allow` | the governance chain let the tool call through |
| `deny` | the chain refused it |
| `pending` | no verdict recorded yet |

### `RunStatus` and `ErrorClass`

`RunStatus` is `headless_runs.status`: `queued` → `preparing` (claimed) → `running` → one terminal state. `RunStatus::is_terminal` is true for `succeeded`, `failed`, `cancelled` and `timed_out`.

| `ErrorClass` | Meaning |
|--------------|---------|
| `ok` | the run succeeded |
| `budget_exceeded` | the turn or spend ceiling stopped the host |
| `timeout` | `wall_clock` elapsed and the process was killed |
| `output_invalid` | the final answer did not satisfy `output_schema` |
| `hook_denied` | a governance hook denied a call the run needed |
| `host_crashed` | the process exited abnormally, or the lease expired on the final attempt |
| `connector_unavailable` | an MCP server or the gateway was unreachable |

`RunOutcome` carries `status`, `error_class`, `session_id`, `cost_microdollars`, `input_tokens`, `output_tokens`, `turns`, `duration_ms`, `structured_output`, `exit_code` and `error`. `execute` returns a `RunReport { outcome, diff, stderr_tail }`, where `diff` is the workspace `DiffSummary` taken before and after the run.

### Lease

| Column | Meaning |
|--------|---------|
| `claimed_by` | the claimer, conventionally `<hostname>:<pid>` (`service::claimer_id`) |
| `lease_until` | the claim expires at this time unless renewed |
| `not_before` | a queued run is not claimed before this time |
| `attempt` | starts at 1, incremented on each reclaim |

- `claim_queued(origin_kind, claimed_by, lease)` takes the oldest due `queued` row of one origin with `FOR UPDATE SKIP LOCKED` and moves it to `preparing`.
- `LeaseRenewer` renews every `DEFAULT_RENEW_EVERY` (30 s) to `DEFAULT_LEASE` (90 s). A failed renewal cancels the run's token.
- `reclaim_stale(origin_kind, max_attempts)` finds `preparing` / `running` rows whose `lease_until` has passed. Below `max_attempts` the row goes back to `queued` with `attempt + 1`, and `request.resume_session_id` is set from `host_session_id` when one was observed. On the final attempt it becomes `failed` / `host_crashed`.
- `finish` writes the outcome only while the row is not yet terminal, and returns whether it did.

## Consumer sketch

A product owns its own table, queues a `headless_runs` row with its `origin_kind` (`HeadlessRunRepository::insert_run` or `insert_run_in_tx` with a `NewHeadlessRun`), and runs a `ClaimLoop` from a `RunnerService`.

```rust
use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_headless::credentials::{IssueRunCredentials, RunCredentialLimits, RunCredentialService};
use systemprompt_headless::execute::{ExecuteParams, execute};
use systemprompt_headless::governance::{HookBases, hooks_file};
use systemprompt_headless::service::{ClaimLoop, ClaimedRun, DbEventSink, RunHandler, claimer_id};
use systemprompt_database::DbPool;
use systemprompt_headless::{HeadlessResult, HeadlessRunRepository, RunLayout, RunRequest};
use systemprompt_provider_contracts::JobContext;
use systemprompt_traits::RunnerService;
use tokio_util::sync::CancellationToken;

const ORIGIN: &str = "agent_farm_run";

struct FarmRunner;

#[async_trait]
impl RunnerService for FarmRunner {
    fn name(&self) -> &'static str {
        "agent-farm-runner"
    }

    async fn run(&self, ctx: JobContext, shutdown: CancellationToken) {
        let Ok(db) = ctx.get::<DbPool>().cloned() else { return };
        let repo = HeadlessRunRepository::new(&db);
        let handler = Arc::new(FarmHandler {
            credentials: RunCredentialService::new(&db, "https://example.invalid"),
            gateway: "https://example.invalid".to_owned(),
            hooks: HookBases {
                govern_url: "https://example.invalid/api/public/hooks/govern".to_owned(),
                track_url: "https://example.invalid/api/public/hooks/track".to_owned(),
            },
        });
        ClaimLoop::new(repo, ORIGIN, claimer_id("node-1"))
            .run(shutdown, handler)
            .await;
    }
}

struct FarmHandler {
    credentials: RunCredentialService,
    gateway: String,
    hooks: HookBases,
}

#[async_trait]
impl RunHandler for FarmHandler {
    async fn run(&self, claimed: ClaimedRun) -> HeadlessResult<()> {
        let row = &claimed.row;
        let request: RunRequest = serde_json::from_value(row.request.clone())?;
        let layout = RunLayout::new(format!("/var/runs/{}", row.id));
        layout.create().await?;

        let credentials = self
            .credentials
            .issue(IssueRunCredentials {
                user_id: row.user_id.clone(),
                name: format!("farm run {}", row.id),
                ttl: request.wall_clock * 2,
                scopes: Vec::new(),
                plugin_id: request.plugin_id.clone(),
                limits: RunCredentialLimits {
                    max_budget_usd: request.max_budget_usd,
                    max_requests: None,
                },
            })
            .await?;

        let hooks = match (&request.plugin_id, &credentials.hook_token) {
            (Some(plugin), Some(token)) => {
                hooks_file(request.host, plugin, token, &self.hooks, &layout).await?
            },
            _ => None,
        };

        let sink = DbEventSink::new(claimed.repo.clone(), row.id.clone());
        let report = execute(ExecuteParams {
            request: &request,
            layout: &layout,
            credentials: &credentials,
            gateway_base_url: &self.gateway,
            hooks_file: hooks,
            seq_after: None,
            sink: &sink,
            cancel: claimed.run_token.clone(),
        })
        .await;
        self.credentials.revoke(&credentials).await?;
        let report = report?;

        let diff = serde_json::to_value(&report.diff)?;
        if claimed.repo.finish(&row.id, &report.outcome, Some(&diff)).await? {
            // stamp the product's own row (agent_farm_runs) from report.outcome here
        }
        Ok(())
    }
}
```

The extension returns the service from `Extension::runner_services()`:

```rust
fn runner_services(&self) -> Vec<Arc<dyn RunnerService>> {
    vec![Arc::new(FarmRunner)]
}
```

The snippet abbreviates error conversion. A real handler maps its own errors into `HeadlessError`, passes `seq_after` from the last persisted event on a resumed attempt, and reads URLs from its config rather than literals.

## Origins

Every product owns its own rows and holds a foreign key to `headless_runs.id`. `headless_runs` never references a product table; `(origin_kind, origin_id)` points back.

| `origin_kind` | Product | Product row |
|---------------|---------|-------------|
| `agent_farm_run` | Dynapps agent farm | `agent_farm_runs.headless_run_id` |
| `workflow_step` | Astound workflows | the workflow step row |

A `ClaimLoop` claims only its own `origin_kind`, so products sharing a database never take each other's runs.

## Node roles

`RunnerService`s start only on nodes whose `server.role` is `all` or `runner`. A `runner` node serves only its probes and runs no scheduler. Several replicas coordinate through `FOR UPDATE SKIP LOCKED`; each replica runs each service once.

## Run directory

`RunLayout::new(root)` names four directories:

| Path | Contents |
|------|----------|
| `root/home/` | the host's `HOME`; deleted after every attempt |
| `root/workspace/` | the working tree, when the run uses one under the root |
| `root/logs/` | process logs |
| `root/host/` | adapter files: MCP config, governance plugin, Codex config, `home-keep/` |

`home/` is always removed, whatever the outcome. Each path an adapter names in `persistent_home_paths` (Claude Code `.claude/projects`, Codex `.codex/sessions`) is moved to `host/home-keep/` after the exit and restored before the next spawn, so a resume can find its transcript.

Resume after a crash therefore needs the reclaiming node to see the original `run_dir`. That holds when runs are node-affine or `run_dir` is on shared storage. The crate does not provide either.

## Governance

- **Claude Code** gets an http-hooks plugin. `governance::hooks_file` writes `host/governance-plugin/` with a `PreToolUse` hook to the govern endpoint and tracking hooks for the other events, all keyed by `?plugin_id=` and authenticated with the run's hook token. Verdicts appear as `hook` events.
- **Codex CLI** has no hook surface. `hooks_file` returns `None` and the run is governed only at the gateway, through its PAT and gateway session. `HostCapabilities::hooks` is `false` for it.

Every gateway call a host makes carries the run's credentials (`RunCredentials`: PAT, gateway session, optional hook token), so it is attributed to the run's user in the governance spine.

## Modules

| Module | Purpose |
|--------|---------|
| `request` | `RunRequest`, `HostKind`, `PermissionMode`, `McpServerSpec` |
| `event` | `RunEvent`, `RunEventPayload`, `RunEventKind`, `HookDecision`, `RunnerLevel` |
| `outcome` | `RunStatus`, `ErrorClass`, `RunOutcome` |
| `layout` | `RunLayout` |
| `host` | `HeadlessHost`, `HostCapabilities`, `Execution`, `host_for`, the adapters |
| `execute` | `execute`, `execute_with_host`, `ExecuteParams`, `RunReport`, `EventSink`, `DiffSummary` |
| `credentials` | `RunCredentialService`, `IssueRunCredentials`, `RunCredentials`, `RunCredentialLimits` |
| `governance` | `hooks_file`, `hooks_document`, `HookBases` |
| `repository` | `HeadlessRunRepository`, `HeadlessRunRow`, `NewHeadlessRun`, `RunStarted` |
| `lease` | `LeaseRenewer`, `LeaseSpec`, `DEFAULT_RENEW_EVERY`, `DEFAULT_LEASE` |
| `service` | `ClaimLoop`, `RunHandler`, `ClaimedRun`, `DbEventSink`, `claimer_id` |
| `error` | `HeadlessError`, `HeadlessResult` |

## License

Business Source License 1.1. See <https://systemprompt.io> for licensing details.
