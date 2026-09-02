# Changelog

## [0.44.0] - 2026-09-02

### Added

- Every log line is stamped with the replica's `instance=`, and `logs` rows carry the instance id, so a line from a multi-replica deployment says which node produced it.
- `sanitize` is a public module, and `redact_argv` redacts a command line for logging: inline `--flag=value` pairs, the element following a sensitive flag, and any argument holding a URL with embedded credentials. The admin CLI gateway logs the argv it forwards, and that argv can carry a secret as a bare positional.

### Fixed

- `sanitize::is_redacted` classifies a field whose name ends in `_key` or `-key`. It matched `api_key` and `apikey` but not `gemini_key`, so provider-key fields were logged in full.

## [0.30.0] - 2026-08-07

### Breaking

- **Breaking:** `AiRequestListItem` gains `cache_read_tokens` and `cache_creation_tokens`; `AuditLookupResult` gains those two plus `status` and `error_message`. Struct literals need the new fields; the queries populate them from the existing `ai_requests` columns.

### Changed

- `list_traces` with `include_system` unset also excludes zero-content traces (no AI requests, no MCP calls, no agent). Bridge housekeeping endpoints mint a log-only trace every few seconds per connected bridge; `TraceListFilter::include_system` remains the escape hatch.

## [0.25.0] - 2026-07-27

### Added

- `render_service_table_into` renders the service-status table into any `std::io::Write`, so its frame geometry can be asserted. `render_service_table` is now a thin wrapper that writes to stdout.

### Fixed

- `AiRequestInfo::provider` and `AiRequestInfo::model` are `Option<String>`, and trace output renders `-` for a request that was rejected before routing resolved either. They previously could not represent such a row at all.
- Integer and boolean fields are no longer redacted by name in either log sink. Redaction matched the substring `token` against the field name with no type check, so a `u64` delete count named `oauth_tokens` was recorded as the string `"[REDACTED]"` — losing the value and changing the field's JSON type. String and debug fields are unaffected.
- The services table renders a square frame. Its width constant was two characters wider than a row's true interior, so the top border overhung every line beneath it and the title row's right edge landed a character short of both. The status column is also padded before it is styled — padding a string that already carried ANSI escapes counted the escape bytes as content and collapsed the column — and the `Port` heading is right-justified to match its values.

## [0.24.0] - 2026-07-26

### Added

- `AiRequestListItem` and `AiRequestDetail` carry the caller's `user_id` (typed `UserId`) along with `actor_kind` and `actor_id`, all read from the columns `ai_requests` already stores. The `infra logs request` read path was the only query module in `trace/` that dropped identity at the SQL boundary.
- `AiRequestFilter::with_user` filters a listing to one user id (exact match, served by the existing `(user_id, created_at)` index).

### Changed

- **Breaking:** `TraceQueryService::list_ai_requests` takes a single `&AiRequestFilter` instead of four positional arguments, matching `list_traces` and `list_tool_executions`. Migrate by building the filter: `AiRequestFilter::new(limit).with_model(pattern)`.

## [0.23.0] - 2026-07-23

### Added

- `LogThrottle`, an interval-based gate for repeated hot-path log emissions.

## [0.21.1] - 2026-07-17

### Changed
- Source files now carry a Business Source License 1.1 header referencing <https://systemprompt.io>.

## [0.20.0] - 2026-07-15

### Removed

- The interactive prompt/summary display stack (`Prompts`, `PromptBuilder`, `QuickPrompts`, `ModuleDisplay`, `BatchModuleOperations`, `ValidationSummary`/`OperationResult`/`ProgressSummary`) and the `dialoguer` dependency; nothing in this repo or any downstream consumer used them.

## [0.19.0] - 2026-07-02

### Breaking

- The minimum supported Rust version is 1.94.
- SQLx is upgraded to 0.9.
- The trace query surface exposes typed trace and AI-request identifiers, and the `LogService` implementation follows the renamed trait lookups: `list_recent` / `find_by_id` (were `get_recent` / `get_by_id`).

## [0.16.0] - 2026-06-22

### Breaking

- The minimum supported Rust version is 1.88.

### Changed

- Over-long functions were split into focused helpers to satisfy the workspace's 75-line function ceiling. No behavioural or API change.

## [0.14.0] - 2026-06-01

### Changed

- Workspace version bump; no API changes in this crate.

## [0.13.1] - 2026-06-01

### Changed

- Workspace version bump; no API changes in this crate.

## [0.13.0] - 2026-05-28

### Changed

- Workspace version bump; no API changes in this crate.

## [0.12.0] - 2026-05-27

### Changed

- Workspace version bump; no API changes in this crate.

## [0.11.0] - 2026-05-20

### Changed
- Workspace-aligned release. Logging surface unchanged; structured fields on the new replica-identity, outbox, and scheduler advisory-lock log sites follow the existing `tracing` conventions.

## [0.9.2] - 2026-05-14

### Changed
- Normalised CHANGELOG to the workspace consumer-facing format.

## [0.1.21] - 2026-04-01

### Changed
- Replaced `OnceLock`-based subscriber initialisation with `ProxyDatabaseLayer` so `init_logging` and `init_console_logging` compose in any order.
- Unified subscriber setup behind `ensure_subscriber` so both init paths register the same registry with fmt and proxy layers.
- Extracted span and event field helpers into `layer/proxy`.

### Fixed
- Surface errors from `DatabaseLayer::flush` instead of silently dropping them when the `logs` table is missing.

## [0.1.18] - 2026-03-27

### Changed
- Upgraded to the Rust 2024 edition.
- Simplified field extraction in the tracing visitor.

## [0.1.2] - 2026-02-03

### Changed
- Switched trace queries to `cost_microdollars` for cost tracking.
- Regenerated the SQLx offline query cache.

## [0.1.0] - 2026-02-02

### Changed
- First stable release at workspace-aligned `0.1.0`.

## [0.0.13] - 2026-01-27

### Changed
- Version bump for workspace alignment.

## [0.0.11] - 2026-01-26

### Added
- `CliService::profile_banner` for printing the active profile to stderr.
- Error messages are now attached to MCP execution trace events for failed tool calls.

### Changed
- Tightened CLI service output and prompt handling.

## [0.0.3] - 2026-01-22

### Changed
- Marked the logging extension as required via `Extension::is_required`.

## [0.0.2] - 2026-01-22

### Changed
- Moved schema registration to the per-crate `Extension` trait and dropped the centralised loaders in `systemprompt-loader`.

### Fixed
- Corrected `include_str!` paths so the crate builds standalone from crates.io.

## [0.0.1] - 2026-01-21

### Added
- Initial release.
