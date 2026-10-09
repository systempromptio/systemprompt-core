# Changelog

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
