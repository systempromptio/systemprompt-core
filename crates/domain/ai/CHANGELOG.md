# Changelog

## [0.14.1] - 2026-06-02

### Added

- The gateway **safety-scanner extension point**. The `SafetyScanner` trait (with `Finding` / `Severity`), the built-in `HeuristicScanner` and `NullScanner`, the `SafetyScannerRegistration` inventory type, and the `register_safety_scanner!` macro now live in this crate's `services::gateway::safety` module and are re-exported at the crate root. Extensions register a scanner the same way they register gateway upstreams or marketplace filters; the consuming gateway resolves the scanner names a policy selects against the built-ins plus every registration. Scanners operate on the `systemprompt-models` canonical request/response types.

### Removed

- The unenforced `max_input_tokens_per_call` and `max_tool_depth` fields are dropped from `GatewayPolicySpec`. They were never applied; `quota_windows` and `safety` are unchanged.

## [0.14.0] - 2026-06-01

### Breaking

- `AiService::new` takes the resolved `&ProviderRegistry`: `AiService::new(&db_pool, &registry, &ai_config, tool_provider, session_provider)`. The provider and image-provider factories build clients from registry entries, and the AI config types are consumed from `systemprompt-models` rather than redeclared in this crate.

### Changed

- Provider drivers map to and from the canonical model through a new `canonical_bridge`, which owns the per-provider sampling and reasoning policy (Anthropic extended-thinking, OpenAI reasoning effort, streaming temperature defaults) and assembles the canonical request the relocated `wire::*` codecs consume. The per-provider request builders and message-model conversion modules they replaced are removed, along with the now-unused legacy Gemini provider structs.

## [0.13.1] - 2026-06-01

### Changed

- Workspace version bump; no API changes in this crate.

## [0.13.0] - 2026-05-28

### Removed

- `AiRequestRecord::minimal_fallback` is deleted. Construction failures propagate to the caller, which logs and skips persistence rather than writing a record with a fabricated `user_id`.

### Changed

- `ImageGenerationRequest.user_id` is now non-optional. Callers that cannot supply a `UserId` were never authorised to generate images.

## [0.12.0] - 2026-05-27

### Changed

- Workspace version bump; no API changes in this crate.

## [0.11.0] - 2026-05-20

### Breaking
- AI gateway tenancy removed. Migration `003_drop_runtime_tenancy.sql` drops the `tenant_id` column from every `gateway_*` table. Repository signatures, request/response types, and the new `services/gateway/` module no longer carry a tenant parameter. Tenancy continues to live in the cloud deployment plane.

### Fixed
- Migration `003_drop_runtime_tenancy.sql` now guards the post-`DROP COLUMN` `ADD CONSTRAINT` statements with an `information_schema.table_constraints` check, so re-running the migration is idempotent on a database that already applied a prior revision. Operators upgrading mid-cycle should run `infra db migrate-repair --apply` to reconcile the resulting checksum drift.

### Added
- `services/gateway/` module hosting the gateway pipeline now that tenancy has been removed from request routing.

### Changed
- Gateway repositories use compile-time-verified `query!` / `query_as!` / `query_scalar!` macros instead of dynamic `query(_)` + `bind(_)`.
- OpenAI image provider and `Authorization` call sites cleaned up of `clippy::useless_borrows_in_formatting`.

## [0.10.2] - 2026-05-15

### Added
- Resilience layer around every provider call: a per-attempt timeout, retry with
  exponential backoff and jitter, a circuit breaker, and a concurrency limit,
  configured via `AiProviderConfig.resilience`.
- `AiError::HttpStatus`, `Timeout`, `CircuitOpen`, and `DependencyUnavailable`
  variants, plus `AiError::classify` distinguishing transient from permanent failures.

### Changed
- Provider HTTP clients now always apply a request and connect timeout; a hung
  connection can no longer block a request indefinitely.
- Non-success provider responses now produce `AiError::HttpStatus` carrying the
  status code and any `Retry-After` header, instead of a flattened `Internal` string.

## [0.9.2] - 2026-05-14

### Changed
- Normalized changelog formatting to match the consumer-facing house style.

## [0.3.0] - 2026-04-22

### Changed
- **Breaking:** `AiQuotaBucketRepository::increment` now takes an `IncrementParams` struct grouping `tenant_id`, `user_id`, `window_seconds`, `window_start`, and `delta`. Migrate by constructing `IncrementParams` at the call site.
- **Breaking:** `AiRequestPayloadRepository::upsert_request` and `upsert_response` now take an `UpsertPayloadParams` struct grouping `body`, `excerpt`, `truncated`, and `bytes`. Migrate by constructing `UpsertPayloadParams` at the call site.

### Added
- Re-exported `IncrementParams` and `UpsertPayloadParams` from the crate root.

## [0.1.3] - 2026-03-20

### Added
- Typed `OpenAiStreamChunk`, `OpenAiStreamChoice`, and `OpenAiStreamDelta` structs for OpenAI streaming.
- Pricing-based cost calculation in `StreamStorageWrapper` driven by `ModelPricing`.
- Token usage accumulation during streaming covering input, output, total, cache read, and cache creation tokens.

### Changed
- **Breaking:** Provider streaming implementations now return `StreamChunk` instead of raw strings. Migrate by matching on `StreamChunk` variants in stream consumers.
- `StreamStorageWrapper` captures token usage and finish reason from `StreamChunk::Usage` during streaming.
- `capture_usage` now accepts a `StreamChunk` directly instead of individual parameters.
- OpenAI streaming parser uses typed `OpenAiStreamChunk` in place of `serde_json::Value`.

## [0.1.2] - 2026-02-03

### Added
- `StreamStorageWrapper` for capturing and storing streaming AI response data.
- Request storage tracking on `generate_stream` and `generate_with_tools_stream`.

### Changed
- **Breaking:** Cost tracking field renamed from `cost_cents` (INTEGER) to `cost_microdollars` (BIGINT) for sub-cent precision. Migrate by reading the new column and dividing by 1_000_000 to recover dollars.
- `RequestStorage` now implements `Clone` to support stream-wrapper ownership.
- Regenerated SQLx offline query cache.

## [0.1.0] - 2026-02-02

### Added
- Anthropic web search support via the `web_search_20250305` tool.
- OpenAI web search support.

### Changed
- Updated AI provider model identifiers to the latest published versions.

### Fixed
- Model configs are now selected correctly for image providers and search-capable models.

## [0.0.13] - 2026-01-27

### Changed
- Version bump for workspace consistency.

## [0.0.12] - 2026-01-26

### Fixed
- Gemini Google Search grounding no longer fails with "Function calling config is set without function_declarations"; `tool_config` is omitted from search requests.

## [0.0.11] - 2026-01-26

### Fixed
- Gemini Google Search grounding is now forced via `tool_config` `mode: Any` instead of relying on `AUTO`.

## [0.0.3] - 2026-01-22

### Added
- Migration system infrastructure.

### Fixed
- Schema validation now accepts VIEW-based schemas.

## [0.0.2] - 2026-01-22

### Changed
- Each domain crate now owns its SQL schemas via the `Extension` trait under the distributed schema-registration pattern.

### Removed
- Centralized module loaders from `systemprompt-loader`.

### Fixed
- `include_str!` paths no longer point outside the crate directory, allowing standalone compilation from crates.io.

## [0.0.1] - 2026-01-21

### Added
- Initial release.
