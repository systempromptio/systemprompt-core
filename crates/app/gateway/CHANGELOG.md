# Changelog

## [0.72.0] - 2026-10-22

### Added

- `InboundAdapter::indexes_tool_calls_by_ordinal` (defaulted to `false`); the OpenAI Chat adapter renumbers streamed `tool_calls` from 0.

### Changed

- An Anthropic passthrough forwards the client's history as sent only when it is strictly paired; otherwise the normalised history is sent.

### Fixed

- OpenAI Chat clients receive thinking as `reasoning_content` (buffered and streamed), and `prompt_tokens`/`total_tokens` include cached prompt tokens.

## [0.71.0] - 2026-10-21

### Added

- A provider declared `credential: user` is served only from the calling user's stored key (`user_provider_credentials`), resolved per request; a caller with no stored key gets `403`. Each request records its `billing_source` (`platform` or `user_key`), and `user_key` requests settle quota at zero cost.
- Providers' `user_agent` and `forward_headers` apply to the upstream call; `x-opencode-session` is derived from the gateway session when the client sends none.

### Changed

- **Breaking:** `GatewayRepositories` gains `user_provider_credentials`; `describe_route_match` and `enforce_route_requirements` moved from `service::resolve` to `service::route_match`.
- **Breaking:** governance types are imported from `systemprompt-governance`, and rate windows are charged and evaluated through `GovernanceEngine::charge_and_evaluate`.
- `resolve_upstream` takes an `UpstreamQuery`.

### Fixed

- A request that the quota windows refuse (enforce) or whose quota check fails has its `rate_windows` charge refunded.

## [0.70.0] - 2026-10-18

### Added

- Quota windows accept `calendar: day | week | month` (UTC) and reset at the next calendar boundary.
- `governance.quota.shared_subject_shards` (default 8) spreads organisation and project windows over several bucket rows; admission checks their sum.
- Policies accept `rate_windows: [{ window_seconds, subject, max_requests }]`, counted across replicas and refused with `429` and `Retry-After`.
- `gateway_phase_seconds`, `gateway_streams_in_flight`, `gateway_journal_receipts` and `gateway_streams_aborted_on_shutdown_total` metrics.
- The outbound `traceparent` is propagated to providers, carrying the sampled flag of a valid inbound one.

### Changed

- **Breaking:** `hidden_requires_route` defaults to `true`.
- **Breaking:** a quota window must set exactly one of `window_seconds` (at least 1) or `calendar`; a stored window with neither or both is a malformed policy and denies the request. `MalformedPolicy::error` is a `MalformedSpec`.
- **Breaking:** `PolicyResolver::from_repository` and `GatewayRepositories::new` take a `CacheEpochs`; gateway policies are cached for 5 s in a process with no listening invalidation relay.
- Thought signatures are looked up in one query and only for Gemini-capable routes.
- A request whose quota windows resolve to the same bucket row charges that row once.
- The audit journal is settled once more during shutdown drain.

### Fixed

- When the drain window expires, each open stream ends with one error event in its wire format, and a pass-through stream cut by shutdown or abort ends on a whole SSE frame.

## [0.68.0] - 2026-10-14

### Changed

- Prompt governance builds `PolicyContext` with `roles: &[]`; role-gated rules apply to tool calls, not prompts.

## [0.66.0] - 2026-10-13

### Changed

- Replayed `tool_result` ingest is flat per turn: the per-conversation ingest mark skips the replayed prefix when the pinning block is still at its index (else a full scan), one batched lookup skips blocks whose gateway ingest has already settled, and the mark advances only when every ingest in the turn succeeded. Was `10 + 7(n-1)` queries on turn `n`; now 13.
- Settlement carries the assistant message's `sequence_number` from the admitted message count (journal `Completion.next_message_sequence`, `#[serde(default)]`); a receipt journaled by an older binary records no assistant text rather than re-reading the ledger.

## [0.65.0] - 2026-10-09

### Changed

- One `PolicyResolver` is built per router and shared across requests, so its 60 s cache is hit; the global gateway policy is no longer read from Postgres on every `/v1/messages`. A policy edit takes effect within 60 s.
- Admission writes commit in one transaction: `GatewayAudit::open` stages them and `commit_admission` writes them right before the upstream call, or first thing on any failure path. Staging in memory keeps no pooled connection pinned while quota and extension guards run. The journal lease is taken after the commit, so a lease still implies the row exists.
- The governance decision row for a dispatched request, including a quota warn-mode decision, is written in the admission transaction. A denial before the request row exists still records through its own path.
- Quota admission and settlement write every window's bucket in one statement (`increment_many`); rows lock in a fixed order, so requests sharing buckets cannot deadlock. A window past the first exceeded ceiling is charged and then reversed, so stored totals match the per-window loop.
- `user_contexts` is upserted once per request: `audit.open` skips the repeat when `GatewayRequestContext::context_bound` is true.

### Fixed

- An admission failure whose failure marker also fails to persist returns `DispatchError::PreAudit` (the handler persists the rejection) instead of `Recorded`.

### Breaking

- `GatewayAudit::open` no longer writes; callers outside the dispatch path must call `GatewayAudit::commit_admission` before reading the request row.
- `GatewayRequestContext` gains `context_bound: bool` and `GatewayRepositories` gains `policy_resolver: PolicyResolver`; a struct literal must name them (`false` keeps the old double upsert).

## [0.64.0] - 2026-10-08

### Added

- Deployment selection strategies: a route or `by_scope` chain takes `strategy: ordered | weighted | least_busy` (default `ordered`). `weighted` draws the first attempt by deployment `weight` among healthy deployments; `least_busy` picks the healthy deployment with the fewest in-flight requests in this process. Failover after the first attempt is unchanged. `service::failover::{plan_selection, DeploymentState, DeploymentLoad, InFlight}` are new, and `gateway_deployment_selected_total{route,provider,strategy}` counts selections.
- Context-window pre-check: the request's input tokens are estimated before any upstream call and compared with the deployment's catalog `limits.context_window`. A request that does not fit moves to a fitting `context_fallbacks` entry or is refused. `service::chain_plan::{order_chain, fit_context_window, PlannedChain}` are new, and `gateway_context_fallbacks_total{from,to}` counts moves.

### Breaking

- `GatewayError` gains `ContextWindow(ContextWindowExceeded)` (400); an exhaustive `match` must name it.

## [0.63.0] - 2026-10-07

### Added

- First release. The AI gateway services move here from `systemprompt_api::services::gateway` and the gateway repository bundle from `systemprompt_api::repository::GatewayRepositories`; the gateway-policy spec, YAML loader and ingestion, and the safety-scanner, route-selector and system-prompt-override contracts move here from `systemprompt_ai::services::gateway`. `PolicyResolver` lives in `policies` beside the spec it resolves.
- `GatewayAuditError` types the failures of the audit trail and settlement journal that were previously `anyhow::Error`.
- Ordered multi-deployment failover: a route's `fallbacks: [{provider, upstream_model}]` (replacing `fallback_provider`/`fallback_upstream_model`) are tried in order on 429/5xx/transport failures, each behind its provider's circuit breaker; `service::failover::plan_attempts(&[bool]) -> Vec<usize>` plans the attempts and `gateway_upstream_failovers_total{from,to,reason}` counts each hop.
- Per-scope upstream routing: `by_scope` binds an attributed scope value to its own deployment chain; an unmapped value is refused unless `unmapped: shared`. `service::resolve::describe_route_match` takes a `scope` argument.
- Scope attribution: `GatewayRequestContext` gains `attribution: RequestAttribution` and `api_key_windows`, and `GatewayRepositories` gains `subject_providers: SubjectProviderSet`. `GatewayRepositories::new` takes a `BackgroundTasks`.
- Quota reservation: `quota::precheck_and_reserve(repo, ReserveParams { .. })` returns `ReserveOutcome::{Admitted, Denied}` and `quota::{settle, release}` true up or release it through `GatewayAudit::{set_quota_reservation, settle_quota, fail_with_usage}`; `QuotaDecision` and `QuotaExceeded` carry `QuotaDetail` for the machine-readable `429`, and windows may key on `api_key` or a scope dimension.
- Safety: `safety.scanner_settings.<scanner>.{fail_mode, timeout_ms, config}` (factories receive `&ScannerSettings`; `ScanError::TimedOut`), and `safety.redact_categories`, which rewrites a finding's spans as `[REDACTED:<category>]` instead of refusing (`Finding` gains `spans` and `replacement`).
- Latency histograms `gateway_overhead_seconds` and `gateway_upstream_duration_seconds` (`OVERHEAD_BUCKETS`, `UPSTREAM_BUCKETS`), recorded in `GatewayAudit::complete`.
- `UpstreamError::Status.request_id` is `Option<ProviderRequestId>`; `PolicyConfigurationError::Invalid` is replaced by `UnknownExemptScope`, `UnusableCondition` and `ToothlessSecretScan`.

### Fixed

- A route matches a provider model by id, alias or upstream name, and a dispatch is priced from its selected route's `pricing:` or the serving provider's catalog.
