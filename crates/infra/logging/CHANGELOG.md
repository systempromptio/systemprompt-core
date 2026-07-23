# Changelog

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
