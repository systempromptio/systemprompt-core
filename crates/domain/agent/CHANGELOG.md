# Changelog

All notable changes to `systemprompt-agent` are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.12.0] - 2026-05-27

### Changed

- Workspace version bump; no API changes in this crate.

## [0.11.0] - 2026-05-20

### Breaking
- Repository signatures across `context/` and `task/` take typed `&ContextId`, `&AgentId`, `&TaskId`, `&MessageId` parameters instead of raw `&str`. Callers must construct identifiers via `Id::new(s)` / `Id::try_new(s)?` rather than passing borrowed strings.

### Added
- A global semaphore bounds in-flight A2A SSE streams using the replica-level concurrency cap supplied by `app/runtime`, so a single replica can't exhaust file descriptors under fan-out.
- Typed notification + webhook path: `apply_notification_status` and `message_exists` consume typed identifiers end-to-end through the handler chain.

### Removed
- Dead `ToolProvider` trait and `AiServiceToolProvider`, which had no callers.

## [0.9.2] - 2026-05-14

### Changed
- Internal error sites converted from `anyhow::Error` to typed `AgentServiceError` / `RepositoryError` variants. No change to the public `AgentError` hierarchy.

## [0.4.3] - 2026-04-29

### Fixed
- Agent subprocesses now inherit `MANIFEST_SIGNING_SECRET_SEED` from the parent process and no longer regenerate the manifest signing seed on launch.

## [0.2.1] - 2026-04-16

### Fixed
- Schema migration `003_a2a_v1_task_states.sql` now updates legacy `'pending'` rows to `'TASK_STATE_PENDING'` and no longer relies on `BEGIN`/`COMMIT` blocks the statement parser ignored.
- Schema migration `004_ai_requests_task_fk.sql` wraps `ADD CONSTRAINT` in an `IF NOT EXISTS` guard so re-runs after a partial failure no longer crash startup.

## [0.2.0] - 2026-04-15

### Breaking
- **Breaking:** `ContextProviderService` now accepts `&UserId`, `&ContextId`, and `Option<&SessionId>`. Migrate by passing borrowed typed identifiers instead of owned strings.
- **Breaking:** `AgentOrchestrationDatabase::mark_failed` and `get_unresponsive_agents` no longer accept their unused parameters. Migrate by removing the `_reason` and `_max_failures` arguments at call sites, including `MonitorService::cleanup_unresponsive_agents`.
- **Breaking:** `should_require_oauth` in the A2A request validation path no longer takes a request argument. Migrate by removing the `&A2aJsonRpcRequest` argument at call sites.

## [0.1.23] - 2026-04-14

### Changed
- A2A request parsing and handler diagnostics now reference method names through the `systemprompt_models::a2a::methods` constants.

### Fixed
- `SendStreamingMessage` requests now open an SSE stream instead of falling through to the non-streaming branch after the A2A v1.0.0 method rename.

## [0.1.21] - 2026-04-02

### Added
- Agent subprocesses inherit `FLY_APP_NAME` from the parent process when set.

## [0.1.6] - 2026-03-20

### Changed
- The AI executor matches typed `StreamChunk::Text` and `StreamChunk::Usage` variants instead of comparing raw strings.

## [0.1.5] - 2026-02-19

### Added
- `Agent` domain model with `from_json_row()` deserialization, `AgentRow` SQLX row, and the `agents` table with JSONB card storage and indexes on `enabled`, `source`, and `name`.
- `AgentRepository` exposing `create`, `get_by_agent_id`, `get_by_name`, `list_all`, `list_enabled`, `update`, and `delete`.
- `AgentIngestionService` for scanning agent directories into the database and `AgentEntityService` as the business-logic wrapper above the repository.

## [0.1.4] - 2026-02-19

### Changed
- Added `server_type` column to the `services` table.

## [0.1.3] - 2026-02-18

### Changed
- `SkillIngestionService` now uses the shared `DiskSkillConfig` and `strip_frontmatter` helpers from `systemprompt-models`.
- The skill registry produces a `"{skill_id} skill"` fallback description in place of an empty string.

## [0.1.2] - 2026-02-17

### Removed
- **Breaking:** Removed the playbook domain (`Playbook`, `PlaybookRow`, `PlaybookMetadata`, `PlaybookRepository`, `PlaybookIngestionService`, `PlaybookService`, and the `agent_playbooks` table). Migrate by moving playbook content into skills and dropping any direct references to the playbook API; the `001_drop_playbooks` migration removes the table on upgrade.

## [0.1.1] - 2026-02-03

### Changed
- `PlaybookIngestionService` scans nested playbook directories without a depth limit and joins path segments with underscores in the generated playbook id.

## [0.1.0] - 2026-02-02

### Changed
- First stable release aligned to workspace version 0.1.0.

## [0.0.13] - 2026-01-26

### Added
- `Playbook` domain type with `PlaybookRow` and `PlaybookMetadata`, backed by `PlaybookRepository` (CRUD plus category filtering), `PlaybookIngestionService` (markdown + YAML frontmatter), `PlaybookService`, and the `agent_playbooks` table.

## [0.0.12] - 2026-01-26

### Fixed
- MCP tool calls with an invalid or stale `context_id` now auto-create a replacement context instead of failing with `Context validation failed`.

## [0.0.11] - 2026-01-26

### Changed
- The A2A request handler forwards the JWT provider into OAuth validation.

### Fixed
- `ArtifactBuilder::build_artifacts()` filters null JSON values so tools returning `Some(Value::Null)` no longer error.

## [0.0.3] - 2026-01-22

### Added
- Migration system infrastructure for the agent schema.

### Fixed
- Schema validation now accepts VIEW-based schemas.

## [0.0.2] - 2026-01-22

### Changed
- Each domain crate owns its SQL schemas via the `Extension` trait; the centralized loader has been removed.

### Fixed
- `include_str!` paths now resolve inside the crate, allowing the crate to build standalone from crates.io.

## [0.0.1] - 2026-01-21

### Added
- Initial release.
