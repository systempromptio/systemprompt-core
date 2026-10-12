# Changelog

## [0.66.0] - Unreleased

### Added

- New crate: a host-generic headless coding-agent executor over one `headless_runs` table shared by every origin.
- Contract types: `RunRequest`, `HostKind`, `PermissionMode`, `McpServerSpec`, the `RunEvent` / `RunEventPayload` / `RunEventKind` event stream with `HookDecision` and `RunnerLevel`, `RunStatus`, `ErrorClass`, `RunOutcome` and `RunLayout`.
- Host adapters behind `HeadlessHost` and `host_for(kind) -> Execution`: Claude Code (`claude -p --output-format stream-json`) and Codex CLI (`codex exec --json`). OpenCode and Hermes resolve to `Execution::Unsupported` with a stated reason.
- `execute` / `execute_with_host`: spawns and supervises the host, pumps decoded events into an `EventSink`, enforces the wall clock and cancellation, snapshots the workspace diff, and always deletes `home/`. Host session stores named by `persistent_home_paths` are kept under `host/home-keep/` across attempts.
- `RunCredentialService`: issues and revokes per-run credentials (PAT, gateway session, hook token) so every gateway call is attributed to the run's user.
- `governance::hooks_file`: writes the http-hooks governance plugin for Claude Code; other hosts are governed at the gateway only.
- `HeadlessRunRepository`: queue insert, `claim_queued` (`FOR UPDATE SKIP LOCKED`), lease renewal, `reclaim_stale` with `resume_session_id` carry-over, event batches, cancel requests and an idempotent `finish`.
- `LeaseRenewer` (30 s renew, 90 s lease) and `ClaimLoop` with `RunHandler`, `ClaimedRun`, `DbEventSink` and `claimer_id`, run from an extension's `RunnerService`.
- `HeadlessExtension` owns the `headless_runs` and `headless_run_events` tables.
