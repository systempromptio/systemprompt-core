# Changelog

## [0.28.0] - 2026-07-31

### Added

- `ai_request_payloads` carries `request_body_sha256`, `prepared_body_sha256`, and `response_body_sha256` (migration `012_payload_digests`). The digest is computed over the full bytes, so a truncated capture remains provable, and the request and prepared digests differ legitimately whenever the gateway retargets the model, clamps `max_tokens`, or strips the caller's end-user identifier.
- `HeuristicScanner` scans `CanonicalRequest.forwarded_surface` at the blocking phase, so content the canonical model cannot represent is no longer invisible to it. Each leaf is scanned as its own unit — concatenating them would let two unrelated strings splice into a match neither one contains.

### Fixed

- A streamed Anthropic completion reports its token usage. The codec turned a `message_delta` frame into either a stop event or a usage event and preferred the stop, so the one frame carrying real counts was discarded and every streamed request recorded zero tokens and zero cost. Both events are now emitted, usage first, so a consumer finalizing on the terminal event has already folded the counts in.
- `StreamStorageWrapper` replaces token counts rather than summing them. Providers report usage as a cumulative snapshot, so adding each frame double-counted any stream that reported usage more than once.
- `canonical_bridge::event_to_chunk` forwards a token count only when it is positive. `CanonicalUsage` has no presence bit, so an unreported field is indistinguishable from a reported zero, and forwarding the zeroes let a frame reporting only `output_tokens` erase an `input_tokens` an earlier frame had supplied. `tokens_used` now includes the cache counts.
- `canonical_bridge::event_to_chunk` forwards each usage count exactly as the frame reported it. It previously dropped any zero to avoid erasing an earlier count, which also discarded genuine zeroes; the presence carried by `CanonicalUsageUpdate` makes the workaround unnecessary.
## [0.27.0] - 2026-07-29

### Breaking

- **Breaking:** quota buckets are keyed by an open-vocabulary subject: `ai_quota_buckets` gains `subject_kind` (default `'user'`) and `cost_microdollars`, `user_id` becomes `subject_id`, and the unique key becomes `(subject_kind, subject_id, window_seconds, window_start)` (migration `011_subject_quota_buckets`; existing rows keep today's behaviour via the default). `IncrementParams` takes `subject_kind`/`subject_id` as plain strings — a subject may be an organization, not a user — and `QuotaBucketDelta`/`QuotaBucketState` gain `cost_microdollars`.
- **Breaking:** `QuotaWindow` gains `subject` (serde default `"user"`; any extension-registered subject-attribute dimension such as `organization`) and `max_cost_microdollars` (a spend ceiling, enforced one request late since cost is known only after the response), and is no longer `Copy`. Policy rows written before this release deserialize unchanged.

### Added

- `USER_QUOTA_SUBJECT`: the default quota-window subject slug.

## [0.25.0] - 2026-07-27

### Breaking

- **Breaking:** `AiRequestRecord::provider` and `AiRequestRecord::model` are `Option<String>`, and the matching `ai_requests` columns are nullable. A request refused before routing has no provider and no model; both columns previously held the literal `"unknown"`, indistinguishable from a provider actually named that. The `NOT NULL` pair becomes a status-keyed `CHECK` — only a `rejected` row may omit them. `AiRequestRecordBuilder::build` is infallible and returns `AiRequestRecord`; `AiRequestRecordError` is deleted. Migration `010` backfills the sentinel to `NULL` and stamps those rows `rejected`.
- **Breaking:** `RequestStatus` gains a `Rejected` variant. Migrate by covering it in any exhaustive match.

### Added

- `SafetyConfig::history` selects how far back the request-phase scanners look: `off` (the default) judges only the newest user turn, `audit` also scans earlier turns and records them at the new `request_history` phase without denying the request, and `block` restores the pre-0.25.0 behaviour. Policies written before the field existed keep working and get `off`.
- `SafetyScanner::scan_request_history` is the seam for judging turns the caller already sent. It defaults to scanning nothing and is called only when policy asks for it, so existing scanners need no change. `PHASE_REQUEST`, `PHASE_REQUEST_HISTORY` and `PHASE_RESPONSE` name the phases a `Finding` can carry.

### Fixed

- `HeuristicScanner` judges the system prompt and the newest user turn rather than the whole conversation. It read `CanonicalRequest::flatten_text`, so every turn re-scanned everything the caller had already sent: a single finding in a blocked category denied every later turn of the conversation, and a tool call the policy layer had correctly refused was replayed into the scan surface as `[tool_use:…]` for the rest of the session.
- The credit-card detector no longer splices unrelated numbers into a card. It stripped every non-digit from the flattened conversation and slid a 16-digit window over the result, so a version string, a timestamp and an identifier could concatenate into a Luhn-valid sequence while nothing present was a card number. Digits are collected per message into runs that only a space or hyphen may interrupt.

## [0.23.0] - 2026-07-24

### Added

- `ai_requests.session_id` carries a foreign key to `user_sessions` (`ON DELETE SET NULL`); the migration nulls historical orphaned rows, and the audit path creates the session row before inserting so a failed request keeps its audit trail.

## [0.21.1] - 2026-07-17

### Changed
- Source files now carry a Business Source License 1.1 header referencing <https://systemprompt.io>.

## [0.19.0] - 2026-07-02

### Breaking

- The minimum supported Rust version is 1.94.
- SQLx is upgraded to 0.9.
- rmcp is upgraded to 2.x; tool content flows through `ContentBlock` in place of the removed `Content`/`RawContent` pair.

### Changed

- `ToolProvider` implementations take a typed `McpServerId` instead of a raw string, and HTTP failures preserve their underlying source instead of flattening to a string.

## [0.17.0] - 2026-06-24

### Added

- `RouteSelector` gateway extension point: the `RouteSelector` trait, `RouteSelectorEngine`, and the `register_route_selector!` / `RouteSelectorRegistration` inventory type let an extension re-route a gateway request programmatically after model-glob and `when`-predicate matching, the same way safety scanners are registered.

### Changed

- `update_completion` records cache accounting on the `ai_requests` row: `UpdateCompletionParams` gains `cache_hit`, `cache_read_tokens`, and `cache_creation_tokens`, which are persisted alongside the existing input/output/total token counts when a request completes.
- `ai_requests` gains a `route_match` column (migration `008_route_match`) recording how a request's gateway route was selected — the matched `when` predicates and/or the selector that fired — and `NULL` for a plain model-only match.

### Fixed

- Regenerated the SQLx offline query cache to match the expanded `update_completion` statement and the `route_match` column.

## [0.16.0] - 2026-06-22

### Breaking

- Error enum tuple variants that wrapped a bare message string are now struct variants with a named `message` field; match arms and constructors change from `Error::Foo(msg)` to `Error::Foo { message: msg }`.
- The minimum supported Rust version is 1.88.

### Fixed

- HTTP client-builder failures in the OpenAI image provider and tool-input serialization failures in the Anthropic provider are logged before falling back.

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
