# Changelog

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
