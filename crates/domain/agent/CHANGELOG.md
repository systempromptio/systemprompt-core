# Changelog

## [0.2.1] - 2026-04-16

### Fixed
- **Idempotent migrations.** `003_a2a_v1_task_states.sql`: removed ineffective `BEGIN`/`COMMIT` (statement parser runs each statement individually), added missing `'pending'` → `'TASK_STATE_PENDING'` update. `004_ai_requests_task_fk.sql`: wrapped `ADD CONSTRAINT` in a `DO` block with `IF NOT EXISTS` guard so re-runs after partial failure are safe. Fixes startup crash on existing databases.

## [0.2.0] - 2026-04-15

### Changed (BREAKING)
- `ContextProviderService` impl updated for the new `ContextProvider` trait — methods accept `&UserId`, `&ContextId`, and `Option<&SessionId>`.

### Fixed
- `AgentOrchestrationDatabase::mark_failed` no longer takes an unused `_reason` parameter. `get_unresponsive_agents` no longer takes an unused `_max_failures` parameter. `MonitorService::cleanup_unresponsive_agents` lost its unused `max_failures` parameter in turn.
- `a2a_server::handlers::request::validation::should_require_oauth` no longer takes an unused `_request: &A2aJsonRpcRequest` parameter — the implementation only inspected `state.config.oauth.required`.
- Removed 1 unnecessary path qualification in `services/a2a_server/auth/validation.rs` (`systemprompt_identifiers::UserId::new` → `UserId::new`).

## [0.1.23] - 2026-04-14

### Fixed
- Streaming dispatch regression: `handlers/request/mod.rs` was still comparing against the legacy `"message/stream"` method name after the A2A v0.3.0 → v1.0.0 migration, so correctly-formed `SendStreamingMessage` requests silently fell through to the non-streaming branch and never opened an SSE stream
- Streaming fallback error string no longer references the removed `message/stream` method name

### Changed
- `A2aJsonRpcRequest::parse_request` and all request handler log/error messages now use the `systemprompt_models::a2a::methods` constants as the single source of truth for A2A v1.0.0 method names

## [0.1.21] - 2026-04-02

### Added
- Forward `FLY_APP_NAME` environment variable to agent subprocesses when present

## [0.1.6] - 2026-03-20

### Changed
- AI executor pattern-matches on `StreamChunk::Text` and `StreamChunk::Usage` variants instead of raw strings

## [0.1.5] - 2026-02-19

### Added
- `Agent` domain model as first-class entity with `from_json_row()` deserialization
- `AgentRow` database row struct for sqlx typed queries
- `agents` database schema with JSONB card storage, indexes on enabled/source/name
- `AgentRepository` with full CRUD: create, get_by_agent_id, get_by_name, list_all, list_enabled, update, delete
- `AgentIngestionService` for scanning agent directories and ingesting to database
- `AgentEntityService` as business logic wrapper around repository
- Schema registration for `agents` table in extension

## [0.1.4] - 2026-02-19

### Changed
- Add `server_type` column to `services` schema

## [0.1.3] - 2026-02-18

### Changed
- Replace local `SkillConfig` and `strip_frontmatter` in `SkillIngestionService` with shared `DiskSkillConfig` from `systemprompt_models`
- Replace `unwrap_or_else(String::new)` in skill registry with meaningful `format!("{skill_id} skill")` fallback

## [0.1.2] - 2026-02-17

### Removed
- Remove playbook domain: `Playbook` model, `PlaybookRow`, `PlaybookMetadata`
- Remove `PlaybookRepository` and all playbook CRUD operations
- Remove `PlaybookIngestionService` and `PlaybookService`
- Remove `agent_playbooks` database schema
- Add `001_drop_playbooks` migration to drop playbooks table

## [0.1.1] - 2026-02-03

### Changed
- Support nested playbook directory structures in `PlaybookIngestionService`
- Remove `max_depth` restriction from playbook scanning (now scans all subdirectories)
- Playbook IDs now use underscores for all path separators (`domain_agents_operations`)

## [0.1.0] - 2026-02-02

### Changed
- First stable release milestone
- All crates now at consistent 0.1.0 version

## [0.0.13] - 2026-01-26

### Added
- Add Playbook domain type with `Playbook` model, `PlaybookRow`, and `PlaybookMetadata`
- Add `PlaybookRepository` with full CRUD operations and category filtering
- Add `PlaybookIngestionService` for parsing markdown files with YAML frontmatter
- Add `PlaybookService` for loading playbooks and listing metadata
- Add `agent_playbooks` database schema with category/domain organization
- Register playbook schema in `AgentExtension`

## [0.0.12] - 2026-01-26

### Fixed
- Auto-create replacement context when MCP tool call has invalid/stale context_id instead of failing with "Context validation failed"

## [0.0.11] - 2026-01-26

### Changed
- Pass JWT provider to OAuth validation in A2A request handler
- Use short-form type imports in `task_helper.rs`

### Fixed
- Filter null JSON values in `ArtifactBuilder::build_artifacts()` to prevent errors when tools return `Some(Value::Null)`

## [0.0.3] - 2026-01-22

### Fixed
- Fix schema validation for VIEW-based schemas
- Add migration system infrastructure

## [0.0.2] - 2026-01-22

### Changed
- Implement distributed schema registration pattern
- Each domain crate now owns its SQL schemas via Extension trait
- Remove centralized module loaders from systemprompt-loader

### Fixed
- Fix `include_str!` paths that pointed outside crate directory
- Ensure crate compiles standalone when downloaded from crates.io

## [0.0.1] - 2026-01-21

- Initial release
