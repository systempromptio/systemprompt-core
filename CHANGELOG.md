# Changelog

## [0.10.2] - 2026-05-15

Database lifecycle hardening: transactional migrations, reversible migrations, an AST-based schema linter, a cross-extension table-ownership contract, post-migration seeds, dependency-ordered extension loading, squash tooling, connection retry, and introspectable migration status.

### Added

- **Transactional migrations.** Each migration runs inside a single `BEGIN`/`COMMIT` envelope; on failure the runner issues `ROLLBACK` and does not record the migration, so a partially-applied migration is no longer possible. `Migration::new_no_transaction` opts a migration out of the envelope for statements that cannot run inside a transaction block (for example `CREATE INDEX CONCURRENTLY`).
- **Reversible migrations.** `Migration` gains an optional `down` field; construct with `Migration::with_down(version, name, up, down)`. `MigrationService::run_down_migrations` reverts the most recently applied migrations, and `infra db migrate down <extension> <count>` exposes this on the CLI. Reverting a migration with no `down` SQL fails with `LoaderError::MigrationNotReversible`.
- **Cross-extension table-ownership contract.** `Extension::owned_tables()` declares the tables an extension's schemas create; `Extension::cross_extension_tables()` declares tables owned elsewhere that its migrations may legally `ALTER`. The migration runner rejects an undeclared cross-extension `ALTER` with `LoaderError::CrossExtensionAlterUndeclared`. Both methods default to empty, so existing extensions are unaffected.
- **Post-migration seeds.** `Extension::seeds()` returns idempotent `Seed` values applied after migrations on every boot and intentionally not tracked in `extension_migrations`. Seed SQL is restricted to `INSERT … ON CONFLICT` / `UPDATE` / `MERGE`; `CREATE`/`ALTER`/`DROP` are rejected.
- **Dependency-ordered extension loading.** The extension registry topologically sorts by `Extension::dependencies()` before falling back to `priority()`. A dependency cycle is a boot-time panic with the offending chain; a missing dependency warns and is skipped.
- **`infra db migrate plan`** lists pending migrations without applying them, and **`infra db migrate status`** reports applied and pending migrations plus checksum drift. Both render a text table or, with `--json`, structured output.
- **`infra db migrate squash --extension <id> --through <N>`** concatenates an extension's first `N` migrations into a `000_baseline_v{N}.sql` file and, with `--apply`, retires their bookkeeping rows behind a synthetic version-0 baseline. It is a dry-run by default and refuses to run unless migrations `1..=N` are all already applied.
- **First-connect retry.** The initial database connection retries transient failures (connection refused, the SSL-handshake race, and "starting up") with exponential backoff at 100/200/400/800ms, capped at five attempts. Non-retryable errors such as authentication failures fail immediately; every attempt is logged at `WARN`.

### Changed

- **The declarative schema linter is now parser-based.** `schema_linter` parses each schema with `pg_query` and classifies statements by AST node variant rather than a hand-rolled keyword scanner. It additionally resolves column references in `CREATE INDEX` and view definitions against sibling `CREATE TABLE` statements and rejects unknown columns at lint time (`LintError::UnknownColumn`). This adds a C-toolchain build dependency (`pg_query`/`libpg_query`). Schemas that previously passed the keyword scanner but reference a column not declared in the same extension's schema files will now fail `just lint-schema`.
- **The schema linter permits `DROP` of a stateless derived object** — `VIEW`, `MATERIALIZED VIEW`, `INDEX`, or `TRIGGER` — when guarded by `IF EXISTS`. Such a drop loses no data and is rebuilt by the sibling `CREATE` statement. `DROP TABLE` and `DROP COLUMN` remain rejected in declarative schemas.

### Fixed

- **User-session analytics views install correctly on databases carrying the previous view shape.** PostgreSQL's `CREATE OR REPLACE VIEW` cannot rename or reorder output columns; the views are now dropped with `DROP VIEW IF EXISTS … CASCADE` before recreation, so a column rename no longer fails at install time.

## [0.10.1] - 2026-05-14

### Fixed

- **Pending migrations now run before an extension's declarative schema is installed**, so a legacy database reaches the target table shape before the schema's `CREATE … IF NOT EXISTS` statements run.
- **The CLI degrades gracefully on expired or invalid cloud credentials** instead of failing startup outright.

## [0.10.0] - 2026-05-14

Friction-reduction follow-ups from the 0.9.2 fresh-clone retro plus a structural rule for schema files. Bumped to a minor because schema files now have a hard linter at boot and `SqlExecutor::parse_sql_statements` changes its public return type.

### Breaking

- **Schema files must be purely declarative.** `<crate>/schema/<name>.sql` may contain only `CREATE TABLE IF NOT EXISTS`, `CREATE INDEX IF NOT EXISTS`, `CREATE [OR REPLACE] FUNCTION/VIEW/TRIGGER`, `CREATE TYPE`, `CREATE EXTENSION IF NOT EXISTS`, and `COMMENT ON`. `ALTER`, `DROP`, top-level `DO $$ … $$`, `UPDATE`/`INSERT`/`DELETE`, `TRUNCATE`, `GRANT`, and `REVOKE` are rejected at install time by `schema_linter::lint_declarative_schema` in `crates/infra/database`. Imperative state transitions move to `<crate>/schema/migrations/NNN_<name>.sql` declared via `Extension::migrations()`. The runner applies pending migrations BEFORE executing each extension's schema, so legacy databases reach the target shape before the schema's `CREATE … IF NOT EXISTS` runs. Pre-merge gate: `just lint-schema` (wired into `just check`). See `instructions/information/migrations.md`.

### Changed

- **Breaking — `SqlExecutor::parse_sql_statements` now returns `DatabaseResult<Vec<String>>` instead of `Vec<String>`.** The hand-rolled line scanner in `crates/infra/database/src/services/executor.rs` is replaced with `sqlparser::Parser::parse_sql(&PostgreSqlDialect, …)`. Named dollar-quoted bodies (`$body$ … $body$`) and apostrophe-quoted function bodies are now handled correctly; the previous heuristic only matched `$$`. Unparseable SQL surfaces as `RepositoryError::Internal` rather than silently producing a truncated statement list. The two helper functions `should_skip_line` / `is_statement_complete` are gone. All three call sites (`services/executor.rs:execute_statements_parsed`, `lifecycle/installation/extension.rs:install_extension_schema`, `services/postgres/mod.rs:execute_batch`) propagate the new `Result`; the installation site maps the parse error into `LoaderError::SchemaInstallationFailed`. Schemas under `crates/**/schema/*.sql` are dialect-clean; this is a strict-mode upgrade, not a behavioural regression.
- **`init_credentials_gracefully` no longer pattern-matches `CloudError::CredentialsFileNotFound` directly** (`crates/entry/cli/src/bootstrap.rs`). It calls a new `CloudError::is_missing_credentials_file()` predicate instead, so future renames or refactors of the variant don't silently regress the fresh-clone fallback path (the exact regression class that broke 0.9.1). A matching `CredentialsBootstrapError::is_file_not_found()` is added for symmetry.
- **`instructions/information/crates-publishing.md` leads with `just release patch`** instead of the raw `./scripts/release.sh patch` invocation. The script itself stays gitignored.

### Added

- **`just release [patch|minor|major]` recipe** in the root `justfile`. Validates the bump kind, checks `scripts/release.sh` is present and executable, then delegates. The release script remains local-only; the recipe is the discoverable entry point referenced from the publishing doc.
- **`CloudError::is_missing_credentials_file()` / `CredentialsBootstrapError::is_file_not_found()`** — inherent `const` helpers, no new traits, no dyn overhead. Two regression tests live in `crates/tests/unit/infra/cloud/src/error.rs`.

### Fixed

- **`parse_sql_statements` mishandled named dollar quotes and bare-apostrophe function bodies.** The previous scanner only looked for `$$`, so a `CREATE FUNCTION … AS $body$ … $body$ LANGUAGE plpgsql;` block (or an apostrophe-quoted body) followed by another statement was treated as a single concatenated statement and rejected by sqlx. Three regression tests added to `crates/tests/unit/infra/database/src/services/executor.rs`: named `$tag$` bodies, apostrophe bodies, and malformed SQL surfacing `Err` instead of producing a garbage statement.

## [0.9.2] - 2026-05-12

### Fixed

- **Fresh-clone bootstrap aborted when `.systemprompt/credentials.json` was absent.** `crates/entry/cli/src/bootstrap.rs::init_credentials_gracefully` previously downcast the underlying anyhow error to `CredentialsBootstrapError::FileNotFound`, but the 0.9.1 refactor of `CredentialsBootstrap::init` to return `CloudResult` meant the error reaching the call site was the converted `CloudError::CredentialsFileNotFound` variant — the downcast missed and the CLI failed strictly instead of falling back to `init_empty()`. The graceful wrapper now calls `CredentialsBootstrap::init()` directly and matches on `CloudError::CredentialsFileNotFound` by pattern, removing the brittle dual `downcast_ref` and the unused `init_credentials()` helper.
- **Schema install on a clean database failed on `CREATE TRIGGER` statements.** `SqlExecutor::parse_sql_statements` (`crates/infra/database/src/services/executor.rs`) treated `CREATE TRIGGER` as opening a plpgsql function body, so it kept appending lines until it saw `END;` / `LANGUAGE plpgsql;` — neither sentinel ever appears in a Postgres trigger (triggers always reference a separate function with `EXECUTE FUNCTION foo();`), so the trigger and every subsequent statement got concatenated into one prepared statement that sqlx rejected. The body-detection branch now fires only on `CREATE [OR REPLACE] FUNCTION`; the internal flag was renamed `in_trigger` → `in_function_body` so the misuse is harder to reintroduce. Regression-covered by `crates/tests/unit/infra/database/src/services/executor.rs`.

### Changed

- **Schema-install pipeline overhaul (Phases 1–4).** Single coherent change to how every `Extension` reaches a live Postgres on boot. Production impact: the prod content_ingestion incident where `markdown_content.locale` was missing despite core 0.9 shipping the safety-net ALTER cannot recur — the installer no longer skips idempotent ALTERs on already-existing tables.
  - **Phase 1 — always-run schemas.** `install_extension_schema` (`crates/infra/database/src/lifecycle/installation/extension.rs`) no longer short-circuits when a schema's primary table already exists. Every `SchemaDefinition.sql` runs on every boot. Schemas are expected to be idempotent (`CREATE TABLE IF NOT EXISTS`, `ADD COLUMN IF NOT EXISTS`, `CREATE INDEX IF NOT EXISTS`) — the previous skip silently dropped every post-install ALTER on legacy tenants.
  - **Phase 2 — transactional install + surfaced errors.** All parsed statements for one extension execute inside a single transaction via `db.begin_transaction()`. On per-statement failure the transaction rolls back and the error carries `Statement N/M failed: …\nSQL: <text>` with the offending statement text. The previous batch-then-per-statement fallback (which could commit partial DDL) is gone.
  - **Phase 2 — checksum drift is a hard error.** `MigrationService::run_pending_migrations` (`crates/infra/database/src/lifecycle/migrations.rs`) returns `LoaderError::MigrationFailed` when a stored migration's checksum no longer matches the SQL on disk. New `MigrationConfig { allow_checksum_drift }` and `MigrationService::with_config` let admins explicitly opt out via `systemprompt infra db migrate --allow-checksum-drift`.
  - **Phase 2 — dependency-weight validation.** `ExtensionRegistry::validate_dependencies` (`crates/shared/extension/src/registry/validation.rs`) now requires every declared dependency to have a strictly lower `migration_weight()` than its dependent, surfacing FK-ordering bugs at registry build instead of install. New variant `LoaderError::InvalidDependencyOrdering { extension, extension_weight, dependency, dependency_weight }`.
  - **Phase 3 — schema install is part of `AppContext`.** New builder hooks `AppContextBuilder::with_migrations(bool)` and `with_migration_config(MigrationConfig)` (`crates/app/runtime/src/builder.rs`) run `install_extension_schemas_full` during `build()`. `systemprompt serve` (`crates/entry/cli/src/commands/infrastructure/services/serve.rs`) sets `with_migrations(true)`; the standalone `run_migrations` helper is gone.
  - **Phase 3 — advisory lock around install.** `install_extension_schemas_full` takes Postgres advisory lock `0x73706F6D70740 1` for the duration of the install pass and releases it on completion (and on error). Rolling deploys can no longer race on idempotent DDL.
  - **Phase 4 — `SchemaSource` enum collapsed to `String`.** `SchemaDefinition.sql: String` (was `enum { Inline(String), File(PathBuf) }`); single constructor `SchemaDefinition::new(table, sql)`. Same applies to `SchemaDefinitionTyped` in the typed path. Every production extension migrated.
  - **Phase 4 — dead YAML-loader subsystem removed.** Deleted `crates/infra/database/src/lifecycle/installation/{module.rs,util.rs}`, `crates/app/runtime/src/installation.rs`, `crates/shared/models/src/modules/types.rs`, `crates/shared/models/src/errors/module.rs`. Public types removed: `Module`, `Modules`, `ModuleDefinition`, `ModuleSchema`, `ModuleSeed`, `ApiConfig`, `ModulePermission`, `SeedSource`, `SchemaSource`, `ModuleRuntime`, `ModuleInstaller`, `install_module`, `install_module_with_db`, `install_module_schemas_from_source`, `install_module_seeds_from_path`, `install_schema`, `install_seed`, `ModuleError`. The dead `AppContext::get_provided_audiences` / `get_valid_audiences` / `get_server_audiences` accessors are also gone (zero non-test callers across core, web, and template). `ModuleType` (Regular/Proxy) is preserved by moving it to `crates/app/runtime/src/registry.rs` where its sole consumers live.

### Added

- **`systemprompt infra db doctor`** (`crates/entry/cli/src/commands/infrastructure/db/doctor.rs`). Read-only drift report: lists tables that exist in `information_schema` but are not declared by any registered extension, declared tables absent from the live database, and declared `required_columns` missing from live tables. Text and JSON output via existing `CommandResult` plumbing.
- **`instructions/information/migrations.md`** — workflow doc for shipping additive vs versioned schema changes, advisory-lock behaviour, dependency-ordering rules, and a triage table for common failure modes.

### Fixed

- **Test workspace pool exhaustion under parallel execution.** `crates/tests/unit/domain/analytics/src/repository/costs.rs` opened a 50-connection sqlx pool per test; cargo's default parallel scheduler put ~8 × 50 = 400 connection requests against `max_connections=100`, timing out the late tests with "pool timed out while waiting for an open connection". Tests in that module now serialise against an in-process `tokio::sync::Mutex` gate carried in `Fixture`. Total wall-clock for the 8 tests is 1.25 s, so the serialisation cost is negligible.

## [0.9.1] - 2026-05-12

### Added

- **Handlebars `json` helper** (`crates/domain/templates/src/registry/mod.rs`). Registered on every `TemplateRegistry::new()`. `{{{json field}}}` emits values via `serde_json::to_string`, correctly escaping backslashes, quotes, newlines, and control characters that Handlebars' default HTML escaping leaves intact — required for safe `<script type="application/ld+json">` and other inline-JSON contexts. Non-string values (numbers, bools, objects) round-trip unquoted.

### Changed

- **Cloud credentials bootstrap error is actionable.** `CredentialsBootstrap` now surfaces an operator-targeted message ("tenant pod credentials rejected by api.systemprompt.io … re-run `systemprompt cloud deploy` or set `SYSTEMPROMPT_ALLOW_UNVALIDATED_CREDS=1` to bypass") instead of the bare underlying error string. The redundant inner `map_err` in `validate_with_api` is removed — the outer call site owns the user-facing wording.

### Fixed

- **Test workspace caught up with 0.9.0 i18n.** Updated fixtures in `crates/tests/unit/domain/content/**` (added `locale: LocaleCode::new("en")` to `Content` initializers, `locale: None` to `ContentMetadata`) and `crates/tests/unit/app/generator/src/sitemap{,/xml,_tests}.rs` (added `alternates: vec![]` to all `SitemapUrl` literals; relaxed an XML assertion that hard-matched the old `<urlset>` opening tag, which now declares the `xhtml` namespace for hreflang `<xhtml:link>` alternates).

## [0.9.0] - 2026-05-08

### Changed

- **Marketplace consolidation: skills, agents, and hooks become file-driven first-class entities.** Three new categories (in addition to plugins and MCP servers) are now sourced directly from `<services_root>/{skills,agents,hooks}/<id>/config.yaml` and validated at startup. The DB ingestion hop has been removed and the corresponding tables dropped. RBAC grants for skills and hooks live in `access_control_rules` (the `entity_type` CHECK already accepts `'skill'` and `'hook'`).
  - **Skills →  disk.** `SkillService` now reads `<services_root>/skills/<id>/{config.yaml, SKILL.md}` directly. The A2A processing layer wires `SkillService::new()` with no DB pool; tracking is injected via `with_execution_step_repo`. Deleted: `SkillRepository`, `SkillIngestionService`, `agent::models::Skill`, `agent_skills.sql`, all `app/sync/{local,diff,export}/skills*` modules, and `cli/commands/cloud/sync/skills.rs`. CLI `core/skills/{create,delete,edit,sync,status,…}` are gone; only `list` and `show` remain. Migration `006_drop_agent_skills.sql` drops the table; `task_execution_steps.step_content` keeps `skill_id` as opaque text in JSONB with no FK, so the drop is safe.
  - **Marketplace agents → disk.** `AgentRegistry` (already YAML-driven) is now the sole source of truth for marketplace/persona agents. Deleted: `AgentRepository`, `AgentEntityService`, `AgentIngestionService`, `agent::models::Agent`, `database_rows::AgentRow`, `agents.sql`, all `app/sync/{local,diff,export}/agents*` modules, and `cli/commands/cloud/sync/agents.rs`. CLI core agents subcommand is removed. Migration `007_drop_agents.sql` drops the table; runtime tables (`services`, `agent_tasks`, `task_messages`, `context_agents`) keep `agent_name` as opaque text without FK, so the drop is safe. **A2A runtime agents are unchanged** — they are routed via `AgentRegistry` and never queried the dropped catalog table.
  - **Hooks → first-class.** `bridge_manifest::load_hooks` and the synthetic-plugin writer already read disk hooks; the CLI `core/hooks/{list,validate}` commands are now wired the same way (via `DiskHookConfig`) instead of walking `PluginConfig.hooks`. The `hooks: HookEventsConfig` sub-field has been removed from `PluginConfig` and the offline `plugin generate` no longer emits `hooks/hooks.json` (the bridge writes it from disk + manifest). Existing plugin `config.yaml` files with a `hooks:` section continue to deserialize cleanly — the unknown field is ignored.
- **`AgentRegistry` snapshot is now lock-free.** Replaces `Arc<RwLock<ServicesConfig>>` with `Arc<ServicesConfig>` and removes the unused `reload()` machinery. The lookup methods stay `async` for `AgentRegistryProvider` trait compatibility but their bodies are pure synchronous lookups.

### Added

- **`ContextId::derived_from_gateway_conversation`.** Stable UUID v5 derivation lets the gateway boundary mint a UUID-shaped `ContextId` per conversation without trusting upstream client `x-context-id` headers (which carry client-specific non-UUID values).
- **Multilingual (i18n) support for DB-backed content.** Framework-level primitives for serving content in multiple locales. `LocaleCode` validated newtype (BCP-47-lite) in `systemprompt_identifiers`. `markdown_content` gains a `locale` column with `UNIQUE (slug, locale)` and a dedicated index (migration in `markdown_content.sql`); `Content`, `CreateContentParams`, and the `ContentMetadata` frontmatter struct all carry `locale`. All repository read paths (`get_by_slug`, `get_by_source_and_slug`, `list_by_source`, `list_by_source_limited`) take `&LocaleCode`; new `list_slugs_with_locales_by_source` powers sitemap hreflang pairing. Ingestion reads `locale: <code>` from frontmatter and defaults to `en` when absent.
- **Global `SiteI18nConfig` on `WebConfig`** (`shared/provider-contracts/web_config/i18n.rs`). Declares `default_locale` + `supported_locales` and exposes a `locale_prefix()` helper (`""` for default locale, `/<code>` otherwise). Default-locale URLs keep the unprefixed shape (`/guides/foo`); non-default locales prefix the path (`/es/guides/foo`).
- **Locale-aware prerender pipeline.** `process_all_sources` fans out across `supported_locales × content_sources`; output paths are composed with `locale_prefix`. `PagePrepareContext` and `PageContext` carry `locale` and expose `with_locale()`; static-page prerenderers are invoked once per locale with locale-prefixed `output_path`. The per-row `locale` is injected into the template JSON so templates can render `<html lang>`. Missing translations are omitted entirely for that locale (no page, no sitemap entry, no hreflang alternate).
- **Sitemap hreflang alternates** (`SitemapUrlEntry.alternates`, `xml::SitemapUrlAlternate`). The generated `<urlset>` declares the `xhtml` namespace and each `<url>` emits one `<xhtml:link rel="alternate" hreflang="…"/>` per sibling locale plus an `x-default` link pointing at the default-locale URL.

### Removed

- `crates/domain/agent/src/{repository/content/{skill,agent}.rs, services/{skills/ingestion,agents/{ingestion,agent_entity}}.rs, models/{skill,agent}.rs, database_rows::AgentRow, schema/{agent_skills,agents}.sql}`
- `crates/app/sync/src/{local/{skills_sync,agents_sync},diff/{skills,agents},export/{skills,agents}}.rs` and the corresponding `pub use` re-exports (`SkillsLocalSync`, `AgentsLocalSync`, `AgentsDiffCalculator`, `AgentDiffItem`, `AgentsDiffResult`, `DiskAgent`).
- `crates/entry/cli/src/commands/{cloud/sync/{skills,agents}.rs, core/skills/{create,create_files,create_prompts,delete,edit,status,sync}.rs, core/plugins/generate/hooks.rs}`.
- `PluginConfig.hooks` and the corresponding `hooks_count` field on the CLI `plugin show` output.

## [0.8.0] - 2026-05-07

### Added

- **`GET /v1/bridge/whoami` (`routes/gateway/bridge_whoami.rs`).** Identity envelope for the bridge profile tab. Decodes the bearer JWT via `JwtContextExtractor::decode_for_gateway`, looks the user up via `UserRepository::find_by_id`, and returns `{user_id, email, display_name?, roles}` — only fields the gateway can authoritatively answer. The bridge consumer (`gui/handlers/profile.rs::identity_value`) tolerates the call failing or omitting fields and falls back to its locally verified identity snapshot for `tenant_id` / `provider`. Wired into `gateway_router` alongside the existing `/bridge/profile`, `/bridge/manifest`, and `/bridge/enabled-hosts` routes.
- **Per-user host enable preferences (`bridge_user_host_prefs`).** New table (schema in `crates/domain/oauth/schema/bridge_user_host_prefs.sql`) records which bridge-managed hosts (`claude-code`, `claude-desktop`, `cowork`, `codex-cli`) the user has enabled. `POST /v1/bridge/enabled-hosts` (`routes/gateway/bridge.rs::set_enabled_host`) upserts a row; `GET /v1/bridge/manifest` reads the rows and includes them as `enabled_hosts` in the signed manifest (when no rows exist, all known hosts default to enabled). Bridge-side `agents.json` is now derived from this manifest field on each apply, replacing the previous probe-based migration path.
- **Gateway protocol layer (`crates/entry/api/src/services/gateway/protocol/`).** Replaces the ad-hoc `converter.rs` / `flatten.rs` / `models.rs` / `upstream.rs` / `upstream/sse.rs` / `stream_tap/sse_parser.rs` files with a typed `CanonicalRequest`/`CanonicalResponse`/`CanonicalEvent` model and explicit inbound/outbound adapters. Inbound supports `anthropic_messages` and `openai_responses`; outbound supports `anthropic`, `openai_chat`, and `openai_responses`. Adapters register through `OutboundAdapterRegistration` for static dispatch. `stream_tap` is rewritten on top of the canonical event stream so per-provider SSE parsing no longer leaks into the safety/audit/usage layers.
- **Signed bridge manifest endpoint (`GET /v1/bridge/manifest`).** Returns a typed `SignedManifest` (moved from `bin/bridge/src/gateway/manifest.rs` to `crates/shared/models/src/bridge/`) populated from real data: skills via `SkillRepository::list_enabled`, agents via `AgentRepository::list_enabled`, plugins from on-disk `<system>/services/plugins/<id>/` walks (per-file sha256 + aggregate), managed MCP servers from `ServicesConfig.mcp_servers` filtered by `enabled`, revocations from `user_api_keys` rows where `revoked_at IS NOT NULL`, and `user` via `UserRepository::find_by_id`. Signed via `systemprompt_security::manifest_signing::sign_value` over a JCS canonical view that matches the bridge-side verifier byte-for-byte.
- **OAuth hook-token minting via `client_credentials`.** New `Permission::HookGovern` / `Permission::HookTrack` (hierarchy slot 15), `JwtAudience::Hook`, `JwtClaims.plugin_id`. New `systemprompt_security::auth::hook_token::HookTokenValidator` enforces signature + scope + `plugin_id` for `/api/public/hooks/{govern,track}`. Token endpoint accepts `plugin_id` and `audience` request fields via `ClientTokenOptions`; hook-scoped clients are pinned to `audience=hook`.
- **`POST /v1/bridge/oauth-client`** provisions or rotates the per-tenant OAuth client used for hook-token minting. Returns plaintext `client_secret` once at creation/rotation time. Backed by `provision_bridge_oauth_client` in `crates/domain/oauth/src/services/bridge.rs`.
- **Bridge heartbeat + active-device registry.**
  - New `bridge_sessions` table (`crates/domain/oauth/schema/bridge_sessions.sql`) keyed on `session_id`, with `bridge_version`, `os`, `hostname`, `started_at`, `last_heartbeat_at`, `last_activity_at`, and forwarded/token totals. Two indices on `last_heartbeat_at` for the active-devices query.
  - `BridgeSessionRepository` (`crates/domain/oauth/src/repository/bridge_session.rs`) — `upsert`, `list_active(within)`, `list_active_for_user`, `delete_stale`. All queries via compile-time `sqlx::query!` / `query_as!` macros.
  - `POST /v1/bridge/heartbeat` (`crates/entry/api/src/routes/gateway/bridge_heartbeat.rs`) — JWT-authed; typed `BridgeHeartbeatRequest`; upserts the session row and returns `204 No Content`.
  - Bridge polling loop (`bin/bridge/src/proxy/heartbeat.rs`) — 30 s cadence, spawned next to the existing token-refresh loop. Reuses the proxy's reqwest client and `TokenCache`. On `401` the token cache invalidates so the next tick re-authenticates.
  - `SessionContext::touch_activity()` is called on every successful messages-path forward, so `last_activity_at` reflects real inference traffic rather than just the heartbeat tick.
  - New CLI: `systemprompt admin bridge list [--user-id <id>] [--within-secs <N>]` (default 120 s = 4× heartbeat grace) for operators to list active devices.

### Changed

- **Breaking — `cowork` is renamed to `bridge` everywhere.** Clean cutover, no compatibility shims. A `0.7.x` bridge cannot authenticate against a `0.8.0` gateway and vice versa.
  - HTTP routes: `/v1/cowork/*` → `/v1/bridge/*`, `/v1/auth/cowork/*` → `/v1/auth/bridge/*`.
  - Wire formats: `JwtAudience::Cowork` (`"cowork"`) → `JwtAudience::Bridge` (`"bridge"`); `ClientId::cowork()` (`"sp_cowork"`) → `ClientId::bridge()` (`"sp_bridge"`); `SessionSource::Cowork` → `SessionSource::Bridge`.
  - DB: `cowork_exchange_codes` → `bridge_exchange_codes`. Idempotent `MIGRATION_002_RENAME_COWORK_TO_BRIDGE` added to the OAuth extension; existing deployments rename in place on next bootstrap.
  - Symbol renames across `systemprompt_oauth` (`issue_bridge_access`, `BridgeAuthResult`, `BridgeExchangeCode`, …), `bin/bridge` macros (`bridge_define_id!`, `bridge_define_token!`), and the file moves `services/cowork.rs` → `services/bridge.rs`, `routes/gateway/cowork.rs` → `routes/gateway/bridge.rs`, `commands/admin/cowork/` → `commands/admin/bridge/`.
  - Env vars: `SP_COWORK_*` → `SP_BRIDGE_*`. Config file: `~/.config/systemprompt/systemprompt-cowork.toml` → `systemprompt-bridge.toml`.
  - GitHub workflows, MDM templates, and `documentation/cowork/` → `documentation/bridge/` follow the same rename. Historical CHANGELOG entries are unchanged.
- **Marketplaces as first-class YAML-defined services.** Curated bundles of plugins, skills, MCP servers, and agents are now declared in YAML and validated at startup, mirroring the existing `PluginConfig` pattern.
  - New `MarketplaceConfig` model (`crates/shared/models/src/services/marketplace.rs`) with `MarketplaceConfigFile` wrapper and `MarketplaceVisibility` enum (`Public | Private | Org`). Aggregates plugins/skills/MCP servers/agents by reference only — never inlines them.
  - New typed `MarketplaceId` identifier (`crates/shared/identifiers/src/marketplace.rs`).
  - `ServicesConfig` gains a `marketplaces: HashMap<MarketplaceId, MarketplaceConfig>` field. `validate_marketplace_bindings()` resolves every `plugins.include`, `skills.include`, `mcp_servers`, and `agents.include` reference against the rest of the config and emits `ConfigValidationError::unknown_reference` on misses, so a typo in a marketplace YAML fails fast at startup.
  - Loader auto-discovers `<services>/marketplaces/<id>/config.yaml`, parses each as `MarketplaceConfigFile`, and inserts into `ServicesConfig.marketplaces` with duplicate detection (`ConfigLoadError::DuplicateMarketplace`). Inline declarations in includes also flow through `merge_no_dup`.
  - `Settings::default_marketplace_id: Option<String>` controls which marketplace `/marketplace.json` resolves to (fallback `"default"`).
  - API: `GET /marketplace.json` now serves the typed default marketplace; new `GET /marketplaces`, `GET /marketplaces/{id}`, `GET /marketplaces/{id}/manifest.yaml` for listing, resolved bundles, and raw YAML.
  - CLI: `systemprompt core plugins generate marketplace` is driven from `ServicesConfig.marketplaces` — emits `marketplace-<id>.json` per declared marketplace plus `marketplace.json` for the default.
- **Dynamic registration default for `token_endpoint_auth_method`.** `DynamicRegistrationRequest::get_token_endpoint_auth_method` now defaults to `client_secret_basic` per RFC 7591 §2 instead of returning `Result<_, String>`. Missing/empty values are accepted and defaulted instead of rejected with HTTP 400.
- **Dynamic registration `client_secret` + `registration_access_token` upgraded** from UUID-v4 strings (~122 bits of entropy) to 32-byte URL-safe random (~256 bits).

### Fixed

- **Gateway context-id is now guaranteed for every request, on every protocol.** Before this change, the bridge proxy only derived an `x-context-id` header for paths matching `/messages` or `/v1/messages` and only when the body parsed as Anthropic-shaped JSON; the gateway then hard-rejected anything that arrived without the header. OpenAI Responses traffic via `/responses` (Codex CLI, Gemini-shape clients) and any direct-to-gateway client therefore failed with `400 missing required x-context-id header`.
  - New shared module `systemprompt_models::gateway_hash` (FNV-1a 64-bit, length-prefixed, label-disambiguated) provides `conversation_prefix_hash` and `context_id_from_prefix_hash`. The bridge and the gateway compute the same `ContextId` for the same first turn, deterministically across processes.
  - Gateway `CanonicalRequest::derived_context_id()` flattens `system + first message` into the shared hash, so every inbound adapter (Anthropic Messages, OpenAI Responses, future shapes) gets identical derivation for free.
  - `routes/gateway/messages/extract.rs` switched from hard-fail `require_conversation_binding` to a header-or-derive policy: body parses first, then `x-context-id` is taken from the header if present, otherwise derived from canonical. The defence-in-depth invariant at `services/gateway/service.rs:39` is unchanged.
  - Bridge `proxy/forward.rs` no longer gates context derivation on path. `proxy/session.rs` `PrefixProbe` now recognises Anthropic Messages, OpenAI Chat Completions, and OpenAI Responses shapes and flattens array `content` parts. The bridge cache uses the same shared hash, so bridge-derived and gateway-derived ids never disagree for the same conversation.
  - Tests: `crates/tests/unit/shared/models/gateway_hash.rs` (17 tests covering hash determinism, FNV-1a segment-boundary disambiguation, role/system/content sensitivity, Unicode payloads, lowercase-hex `ctx_*` formatting, 1024-row collision-distribution sanity, and a frozen known-vector that locks the wire-format hash so future algorithm changes become an explicit breaking change) and `crates/tests/unit/bridge/proxy/derive_context_id.rs` (21 tests covering all three protocol shapes, the "second turn rehashes to same id" invariant, **cross-protocol equivalence** — Anthropic system + OpenAI-Chat leading-`role:"system"` message + OpenAI-Responses `instructions` all converge to the same `ContextId` for the same conversation — array-content concatenation, multi-system-message concatenation, default role inference, and resilience to extra unknown fields).
- **Bridge probe extracts inline OpenAI-Chat system messages.** `bin/bridge/src/proxy/session.rs` `PrefixProbe::first_turn` now coalesces leading `role:"system"` messages into the canonical `system` text, so an Anthropic body `{system, messages:[user]}` and an OpenAI Chat body `{messages:[system, user]}` carrying the same conversation hash to the same `ContextId`.
- **`profile_gateway` test crate compiles again.** `GatewayRoute` recently gained a `pricing: Option<ModelPricing>` field; the test fixtures in `crates/tests/unit/shared/models/src/profile_gateway.rs` were missing the initializer and broke the `systemprompt-models-tests` build. Added `pricing: None` to the two literal `GatewayRoute` constructors so the test crate compiles and `gateway_hash` tests can run.
- **`otel.rs` clippy hygiene.** Folded redundant `map(...).unwrap_or(...)` over `Option` into `map_or`, made `severity_to_level` a `const fn`, switched `&Option<AnyValue>` parameters to `Option<&AnyValue>` (Clippy `ref_option`), and dropped the now-unnecessary `#[allow(clippy::ptr_arg)]`. No behavior change.

## [0.7.0] - 2026-05-06

### Added

**Unified authorization decision plane (`crates/infra/security/src/authz/`)**

- **`AuthzDecisionHook` async trait** — single extension point for both the gateway `/v1/messages` proxy and the MCP RBAC middleware. Both enforcement sites call `evaluate(AuthzRequest) -> AuthzDecision` via a process-global slot installed at server startup.
- **`WebhookHook`** — fail-closed production implementation. POSTs `AuthzRequest` to an extension HTTP endpoint (e.g. the template's `POST /govern/authz`). Any transport error, non-2xx response, decode failure, or timeout denies the request and records the fault to the audit sink. There is no fail-open mode.
- **`DenyAllHook`** — bootstrap default and `mode: disabled` implementation. Denies every request and records to the audit sink so outages remain observable.
- **`AllowAllHook`** — dev/test only. Installed only when the operator passes the exact `unrestricted` acknowledgement in the profile; bootstrap fails otherwise. Every call logs an `ERROR` line and writes an audit row so unrestricted operation is never silent.
- **`AccessControlRepository`** — typed queries against `access_control_rules` (`list_rules_for_entity`, `list_rules_bulk`, `upsert_rule`, `delete_rule`, `set_default_included`, `get_default_included`). Generic over `EntityKind`.
- **`resolve(rules, user_id, roles, department, default_included) -> Decision`** — pure deny-overrides resolver with user > role > department > default specificity. Zero DB calls; suitable for unit testing.
- **`EntityKind` enum** (`GatewayRoute`, `McpServer`) — typed entity references in `AuthzRequest`; serializes to `"gateway_route"` / `"mcp_server"` for JSON compatibility with the extension webhook.
- **`GovernanceDecisionRepository` and `DbAuditSink`** — write every authorization decision (allow and deny) to the `governance_decisions` table with `entity_type`, `entity_id`, `user_id`, `tenant_id`, `decision`, and `evaluated_rules`. `NullAuditSink` for tests and pre-database bootstrap.
- **`install_from_governance_config`** — reads `services/governance/config.yaml` (`mode: webhook | disabled | unrestricted`) and installs the process-global hook at startup. Called from `AppContextBuilder::build` after the database pool is created.
- **Schema migrations** embedded via `AuthzExtension`: `access_control_rules` (entity × rule_type × access with deny-overrides precedence) and `governance_decisions` (unified audit log for all authorization decisions).
- **`systemprompt-security-authz-tests` crate** (`crates/tests/unit/infra/security/authz/`) — bootstrap, hook-runtime, webhook-hook, and profile-governance unit tests.

**JWT and profile changes**

- **`JwtClaims.department: Option<String>`** and **`JwtClaims.tenant_id: Option<TenantId>`** — new optional claims skipped during serialization when absent. Populated by the token issuer at login; forwarded to `AuthzRequest` at both enforcement sites without a DB round-trip per request.
- **`GovernanceConfig` and `AuthzMode`** profile types (`crates/shared/models/src/profile/governance.rs`). `AuthzMode` is `webhook | disabled | unrestricted`; `UNRESTRICTED_ACKNOWLEDGEMENT` is the sentinel string that must be set exactly for `AllowAllHook` to install.
- **Stable `id` field on `GatewayRouteView`** (`crates/shared/models/src/profile/gateway.rs`) — slug+hash ID persisted in `profile.yaml`; backfill keeps legacy profiles working without migration.

**External-agent catalog**

- **`ExternalAgentConfig` and `ExternalAgentKind`** types (`crates/shared/models/src/services/external_agent.rs`). Catalog entry for native apps and CLI tools that connect via the bridge binary (Claude Desktop, Codex CLI, Claude Code). Intentionally distinct from `AgentConfig` (server-side A2A agents).
- **`ExternalAgentId`** typed identifier (`crates/shared/identifiers/`).
- **`external_agents` field** wired through `ConfigLoader` (`RootConfig`, `PartialServicesFile`, merge logic) with a `DuplicateExternalAgent` error on name collision across included service files.

### Changed

- **`/v1/messages` gateway enforcement** (`crates/entry/api/src/routes/gateway/messages/extract.rs`): `extract_request_context` refactored into `read_gateway_body` and `build_authz_request` (≤58 lines each); missing `tenant_id` in the JWT now returns 401 instead of constructing an empty `TenantId`; `AuthzDecisionHook::evaluate` is called after JWT/scope validation via `global_hook()`; requests are explicitly denied when no hook is installed.
- **MCP RBAC middleware** (`crates/domain/mcp/src/middleware/rbac.rs`): missing `tenant_id` returns an authz-deny `McpError`; uses typed `EntityKind::McpServer`; `AuthzDecisionHook::evaluate` called after `enforce_rbac_from_registry` succeeds; explicitly denies when no hook is installed.

### Removed

- **`just check-bans` and `just check-bans-crate` recipes** (`justfile`) and the matching `check-bans` job in `.github/workflows/quality.yml`. The recipes were grep-based stand-ins for three rules: raw `String` ID fields, `*Manager` type names, and out-of-allowlist `sqlx::query()`. Typed-ID discipline and the `*Manager` preference remain reviewer-enforced conventions (already documented as such in `CLAUDE.md` and `instructions/prompt/rust.md`); the sqlx allowlist is enforced by clippy and `ci/check-sqlx.sh`. Dropping the recipes removes a governance surface that was producing busywork (23 historical `*Manager` flags across MCP/scheduler/agent internals) without a corresponding bug class. Existing dated audit reports under `instructions/audits/` continue to reference these recipes as historical evidence and are intentionally left unchanged.

## [0.6.0] - 2026-05-05

### Changed

- **Breaking — `DatabaseProvider`, `DatabaseTransaction`, and `DatabaseProviderExt` traits return `DatabaseResult<T>`** (`crates/infra/database/src/services/provider.rs`, `crates/infra/database/src/models/transaction.rs`). Every method that previously returned `anyhow::Result<T>` now returns `Result<T, RepositoryError>`. External crates implementing these traits must update return types and convert their backend errors via `RepositoryError::Database(#[from] sqlx::Error)`, `RepositoryError::Serialization(#[from] serde_json::Error)`, or `RepositoryError::invalid_state` for runtime invariant failures. Migration:
  ```rust
  // before
  async fn execute(&self, ...) -> anyhow::Result<u64> { ... }
  // after
  async fn execute(&self, ...) -> systemprompt_database::DatabaseResult<u64> { ... }
  ```
- **Breaking — `FromDatabaseRow::from_postgres_row` returns `DatabaseResult<Self>`** (`crates/infra/database/src/models/query.rs`). Decoders implementing the trait must return `Result<Self, RepositoryError>` instead of `anyhow::Result<Self>`.
- **Breaking — `Database::new_postgres`, `Database::from_config`, `Database::pool_arc`, `Database::write_pool_arc`, `Database::read_pool_arc`, `Database::begin`, and `PostgresProvider::new`** all return `DatabaseResult<T>` (`crates/infra/database/src/services/database.rs`, `crates/infra/database/src/services/postgres/mod.rs`).

### Added

- **`RepositoryError::InvalidState(String)` variant** plus `RepositoryError::invalid_state(msg)` constructor (`crates/infra/database/src/error.rs`). Captures driver-protocol invariant failures previously wrapped in `anyhow!` (transaction reused after commit, scalar query with no columns, unsupported `DbValue` type).
- **`From<systemprompt_database::RepositoryError> for systemprompt_traits::RepositoryError`** bridge so domain repositories that store the boxed-error variant pick up the typed database error transparently through `?`.
- **`#[from] systemprompt_database::RepositoryError` variants** added to `McpDomainError`, `OauthError`, `UserError`, `FilesError`, and `LoggingError`. Repositories propagating database errors via `?` no longer need a manual `.map_err(...)`.
- **Typed identifiers extended for cloud surfaces** — `TenantId`, `PriceId`, `TransactionId`, `CheckoutSessionId`, `ConnectionId`, `SectionId` now used end-to-end across `crates/infra/cloud/`, `crates/entry/cli/src/commands/cloud/`, and `crates/shared/models/src/api/cloud/**`. Eliminates 50+ raw-`String` ID call sites.
- **`domain_error!` declarative macro** (`crates/shared/models/src/errors/macros.rs`). Domain crates compose their typed error enum from a `common: [repository, io, json, yaml, validation, not_found, config, anyhow, http]` token list plus their own variants. Drops ~300 lines of boilerplate across `files`, `mcp`, etc.
- **`crates/shared/identifiers/src/{cloud,connection,section}.rs`** — new typed-ID modules backing the cloud and dashboard surfaces.

### Removed

- **`impl From<anyhow::Error> for RepositoryError`** legacy bridge (`crates/infra/database/src/error.rs`). The bridge was only required while the trait surface returned `anyhow::Result`; now obsolete.
- **`impl From<anyhow::Error> for UserError`** and **`impl From<anyhow::Error> for LoggingError`** — the trait surface no longer produces `anyhow::Error` to be absorbed.

### Quality

- `cargo check --workspace`: clean.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test --manifest-path crates/tests/Cargo.toml --workspace`: **3578 passed, 0 failed.**
- `cargo sqlx prepare --workspace`: refreshed; `.sqlx/` cache committed.
- **CLAUDE.md** updated to point at canonical `instructions/prompt/rust.md` and to spell out the real comment policy: inline `//` only for non-obvious *why*, `///` not applied mechanically, `//!` on `lib.rs` and significant `pub mod` heads as the load-bearing form, banned in `entry/*` binaries and inside `crates/tests/**`.
- **`rust-coding-standards` skill cache** synced from marketplace source so it no longer says "delete `///`".
- **Lint hygiene** — every hand-written `#[allow(...)]` outside `crates/tests/` (54 sites) now carries a `// reason: ...` comment so external scanners can see the suppression rationale. No allow was removed; no behavior changed.
- **Sqlx allowlist documented** — extended the `sqlx::query(_)` allowlist in `CLAUDE.md` and `justfile` (`check-bans`) to cover `crates/entry/cli/src/commands/admin/setup/**` (bootstrap DDL: `CREATE USER` / `CREATE DATABASE` / `GRANT` / `CREATE EXTENSION`, which cannot bind identifier parameters and run before the target database exists). Each call site now carries an `// allowlist: bootstrap DDL` annotation.

## [0.5.0] - 2026-05-04

### Added

- **`AppPaths` accessor on `AppContext`** (`crates/app/runtime/src/context.rs`). `ctx.app_paths()` returns `&AppPaths` and `ctx.app_paths_arc()` returns `Arc<AppPaths>`. Replaces the deleted `AppPaths::get()` global singleton.
- **`OauthResult<T>` and `FilesResult<T>`** type aliases now exposed by `systemprompt-oauth` and `systemprompt-files` crates. Public-API surface (repositories, services, validators) returns these typed results.
- **`McpDomainResult<T>` and `AgentResult<T>`** type aliases on `systemprompt-mcp` and `systemprompt-agent`. Public-API surface (`McpServerRegistry`, `RegistryManager`, `LifecycleManager`, `ProcessManager`, `DatabaseManager`, `McpOrchestrator`, `AgentRegistry`, `AgentLifecycle`, `validate_agent_binary`) now returns the typed aliases. `McpDomainError` is the public name; `pub use rmcp::ErrorData as McpError` retains the existing `McpError` symbol for tool-call boundary use.
- **`systemprompt_config::load_profile_with_catalog`** — single entry point for loading a profile YAML from disk and resolving its gateway catalog. Lives in `crates/infra/config/src/profile_loader.rs` (with companion `profile_gateway::resolve_catalog`).
- **`crates/infra/config/src/bootstrap/`** module — new home for `SecretsBootstrap`, `ProfileBootstrap`, `manifest_seed`, and the `BootstrapSequence<S>` machinery. The `BootstrapSequence` is now `Uninitialized → ProfileInitialized → SecretsInitialized → BootstrapComplete` (paths state removed).
- **`CategoryIdUpdate` re-export** from `systemprompt-content` for explicit `Unchanged | Clear | Set(CategoryId)` semantics; replaces `Option<Option<CategoryId>>` in the CLI content-edit state.

### Changed

- **Breaking — `AppPaths` is no longer a global singleton.** `AppPaths::init` and `AppPaths::get` are deleted. `AppPaths::from_profile(&profile.paths)` is the sole constructor. Components that previously called `AppPaths::get()` now receive `&AppPaths` (or `Arc<AppPaths>`) explicitly: 42 call sites across `infra/`, `domain/`, `app/`, `entry/`, and `crates/tests/` were rewritten. `JobContext` carries `app_paths` as a type-erased `Arc<dyn Any + Send + Sync>` (parallel to `db_pool` and `app_context`) so generator/sync jobs can downcast without depending on `systemprompt-runtime`.
- **Breaking — bootstrap I/O moved out of `systemprompt-models`.** `SecretsBootstrap`, `ProfileBootstrap`, `manifest_seed`, and the `BootstrapSequence<S>` machinery now live in `systemprompt-config`. `Secrets::load_from_path` is replaced by free function `systemprompt_config::load_secrets_from_path`. `Config::try_init` / `Config::init` / `Config::from_profile` are replaced by `systemprompt_config::{try_init_config, init_config, init_config_from_profile, build_from_profile}`. `Config::is_initialized` / `Config::get` / `Config::install` remain on the type. `validators::skills::SkillConfigValidator` moves to `systemprompt_config::SkillConfigValidator`. ~110 import sites updated; 14 crates picked up a `systemprompt-config` dependency. Restores the `crates/shared/models/` "no I/O" invariant from `boundaries.md`.
- **Breaking — public APIs in `systemprompt-oauth` and `systemprompt-files` return typed `Result`.** `OAuthRepository::*`, `validate_jwt_token`, `SessionCreationService::create_anonymous_session` return `OauthResult<T>`. `FileRepository::*`, `FileService::*`, `AiService::*`, `ContentService::*` (in files crate), and `FilesAiPersistenceProvider::new` return `FilesResult<T>`. `#[from] sqlx::Error`, `#[from] anyhow::Error`, and `#[from] std::io::Error` adapters provide compatibility for internal helpers that still return `anyhow::Result`.
- **Breaking — public APIs in `systemprompt-mcp` and `systemprompt-agent` return typed `Result`.** Registry, lifecycle, process, database, and orchestrator surface methods now return `McpDomainResult<T>` / `AgentResult<T>`. Internal helpers and upstream trait impls (`McpRegistryProvider`, `AgentRegistryProvider`) keep `anyhow::Result`; `#[from] anyhow::Error` adapter bridges the boundary.
- **Breaking — `Profile::parse` removed; replaced with `Profile::from_yaml`.** `from_yaml` does pure YAML deserialization with no I/O. Gateway catalog resolution moved to `systemprompt_config::profile_gateway::resolve_catalog`. The single user-facing entry point is `systemprompt_config::load_profile_with_catalog(path)`. Restores the `crates/shared/models/` "no I/O" invariant for the profile module.
- **`bin/bridge` pins `systemprompt-identifiers = "0.5.0"`** with `path` override, so bridge resolves cleanly both locally and from crates.io once 0.5.0 ships.
- **`ProxyError::AuthChallenge(Box<Response<Body>>)`** — variant now boxes the `axum::Response` to satisfy `clippy::result_large_err`. Internal-only change; constructor now wraps with `Box::new`.

### Removed

- **Breaking — `AppPaths::get()` and `AppPaths::init`** from `crates/shared/models/src/paths/mod.rs`. Use `AppPaths::from_profile` and pass the value through `AppContext` or function arguments.
- **`PathError::NotInitialized` and `PathError::AlreadyInitialized`** variants — the singleton states they described no longer exist.
- **`BootstrapSequence::with_paths`, `with_paths_config`, `skip_paths`, `presets::full`, `PathsInitialized`** — paths are now built from the profile in the `AppContext` builder; no separate bootstrap step.
- **Re-exports of `SecretsBootstrap`, `ProfileBootstrap`, and `manifest_seed`** from `systemprompt-models`. Import from `systemprompt-config` instead.

### Quality

- `cargo clippy --workspace -- -D warnings`: clean (eliminated 12 pre-existing pedantic lints in CLI and proxy code: `result_large_err`, `option_if_let_else`, `needless_pass_by_value`, `option_option`, `assigning_clones`, `bool_to_int_with_if`, `manual_let_else`, `needless_borrow`). Closed 3 remaining lints in `systemprompt-test-mocks` (`type_complexity` x2, `derivable_impls` x1).
- `cargo test --manifest-path crates/tests/Cargo.toml --workspace`: **8984 passed, 0 failed, 0 ignored.** Repaired bridge-* test crates (async migration, `Cell` → `Mutex` for `Send + Sync`, `ureq` → `reqwest` mock construction, removed-module deletions, env-var renames). Updated migration-weight assertions to match the v0.4.4 weight re-spacing. `events-tests` and `concurrency-tests` migrated to bounded `mpsc::channel(SSE_BUFFER)`.

## [0.4.4] - 2026-05-03

### Added

- **Code-quality remediation pass** addressing findings from the v0.4.3 ruthless review:
  - **Granular facade features** in `systemprompt/Cargo.toml` — `logging`, `config`, `loader`, `events`, `client`, `security` are now individually selectable instead of being bundled only under `full`. Backwards-compatible: `full` still enables them all.
  - **`OauthError` and `FilesError` thiserror enums** (`crates/domain/oauth/src/error.rs`, `crates/domain/files/src/error.rs`) with `#[from] sqlx::Error`, `#[from] anyhow::Error`, and `#[from] std::io::Error` conversions. Public APIs can now expose typed errors at boundaries instead of opaque anyhow strings; existing internal anyhow remains and migrates incrementally.
  - **Migration weight headroom** — extension `migration_weight()` values re-spaced ×10 (database 1→10, users 10→100, scheduler 55→550, etc.). Reserved ranges going forward: 0–99 infra core, 100–199 shared platform, 200–999 domain, 1000+ third-party extensions.
- `crates/entry/api/src/services/gateway/captures.rs` — leaf module exposing `CapturedToolUse` and `CapturedUsage` so `audit.rs` and `parse.rs` no longer import each other.
- `crates/entry/cli/src/commands/admin/setup/common.rs` — leaf module with `PostgresConfig`, `generate_password`, `detect_postgresql`, `test_connection`, `enable_extensions`. Removes the back-edge from `postgres.rs` to `docker.rs`.
- `bin/bridge/src/gui/emit.rs` — leaf module with all `emit_*`, `send_emit`, and `send_reply*` helpers. Breaks the `command.rs ↔ ipc_runtime.rs` cycle.
- `.sentrux/rules.toml` and `.sentrux/baseline.json` — structural-quality gates for future agent sessions (`sentrux check` / `sentrux gate`).

### Changed

- **Refactor — bridge GUI command dispatcher** (`bin/bridge/src/gui/command.rs::dispatch`, cc 61 → ≤25). Split the 25-arm string match into family routers (`meta`, `gateway`, `auth`, `sync`, `host`, `agent`, `diagnostics`) chained via `Option<CommandOutcome>`.
- **Refactor — bridge GUI event dispatcher** (`bin/bridge/src/gui/dispatch.rs::dispatch`, cc 32 → ≤10). Split into `dispatch_window`, `dispatch_request`, `dispatch_finished`, `dispatch_lifecycle`, `dispatch_ipc` chained by `Result<(), UiEvent>`.
- **Refactor — bridge GUI event-kind tracer** (`bin/bridge/src/gui/dispatch.rs::event_kind`, cc 30 → ≤10). Bucketised into `request_kind`, `finish_kind`, `lifecycle_kind`, `ipc_kind`.
- **Refactor — startup-event renderer** (`crates/entry/cli/src/presentation/renderer.rs::handle_event`, cc 32 → ≤10). Split into `handle_phase_event`, `handle_service_event`, `handle_status_event`, `handle_terminal_event`.
- **Refactor — proxy auth validator** (`crates/entry/api/src/services/proxy/auth.rs::validate`, cc 33 → ≤8). Extracted `lookup_oauth_requirement`, `resource_path_for`, `mcp_session_fallback`, `challenge_or_error`, `ensure_required_scopes`.
- **Refactor — agent edit CLI** (`crates/entry/cli/src/commands/admin/agents/edit.rs::execute`, cc 37 → ≤6). Field-update logic moved to `apply_enabled_flags`, `apply_runtime_fields`, `apply_card_fields`, `apply_capability_fields`, `apply_metadata_fields`, `apply_mcp_server_changes`, `apply_skill_changes`, `apply_set_value_changes`.
- **Refactor — content-types edit CLI** (`crates/entry/cli/src/commands/web/content_types/edit.rs::execute`, cc 30 → ≤6). Extracted `apply_basic_flags`, `apply_sitemap_flags`, `apply_set_value_changes`, `apply_set_key`, `apply_sitemap_set`.
- **Refactor — content edit CLI** (`crates/entry/cli/src/commands/core/content/edit.rs::execute_with_pool`, cc 28 → ≤6). Introduced `ContentEditState` builder and per-field appliers.
- **Refactor — services cleanup CLI** (`crates/entry/cli/src/commands/infrastructure/services/cleanup.rs::execute`, cc 26 → ≤8). Extracted `no_services_result`, `dry_run_result`, `stop_running_services`, `stop_api_server`, `format_cleanup_message`.
- **Refactor — cloud status CLI** (`crates/entry/cli/src/commands/cloud/status.rs::execute`, cc 38 → ≤8). Split into `load_profile_info`, `load_credentials_and_tenants`, `render_status`, `render_profile`, `render_credentials`.
- **Refactor — keyword-table conversions**. Replaced six long if-else / match chains with static lookup slices: `parse_browser` / `parse_os` (`user_agent.rs` cc 44 → ≤4), `Validator::get_extension` (`upload/validator.rs` cc 43 → ≤3), `is_scanner_agent` (`scanner.rs` cc 41 → ≤6), `detect_mime_type` (`core/files/upload.rs` cc 35 → ≤3), `filter_log_events` (`ai_trace_display.rs` cc 26 → ≤6).

### Fixed

- 3 structural import cycles eliminated (gateway audit↔parse, setup docker↔postgres, bridge command↔ipc_runtime). 6 → 3 cycles reported by Sentrux; the remaining 3 (gemini params↔tools, gateway extract↔webauthn authenticate, bridge auth↔gateway_probe) are tree-sitter resolver false positives — neither file imports back from the other.

### Quality

- Sentrux structural-quality score: **5299 → 5935**, `sentrux check ✓ All rules pass` (`max_cycles=3`, `max_cc=38`, `no_god_files=false`).
- 16 functions exceeded cc=25 before; only `bin/bridge/web/js/components/sp-host-card.js::render` (cc=38) remains, intrinsic to its multi-state HTML template.

## [0.4.3] - 2026-04-29

### Added

- `JwtAudience::Cowork` variant in `crates/shared/models/src/auth/enums.rs` (`as_str` and `FromStr` covered).
- `SecretsBootstrap::manifest_signing_secret_seed() -> Result<[u8; 32], _>` accessor in `crates/shared/models/src/secrets_bootstrap.rs`.
- `manifest_signing::sign_value<T: Serialize>` and `canonicalize<T>` in `crates/infra/security` for RFC 8785 (JCS) canonical JSON.
- `systemprompt admin cowork rotate-signing-key` CLI generates a fresh ed25519 seed, persists it, and prints the resulting base64 pubkey.

### Changed

- **Breaking**: `issue_cowork_access_with` (`crates/domain/oauth/src/services/cowork.rs`) mints `audience: vec![JwtAudience::Cowork]` instead of `JwtAudience::Api`. A cowork JWT no longer authorises generic API endpoints.
- Manifest signing seed is now a dedicated 32-byte value persisted under `manifest_signing_secret_seed` in the secrets file, generated by `OsRng` on first bootstrap. Replaces the prior `Sha256(DOMAIN_SEPARATOR || jwt_secret)` derivation in `crates/infra/security/src/manifest_signing.rs::signing_key`. JWT HMAC compromise no longer compromises manifest signatures.

### Fixed

- `Secrets::parse` (`crates/shared/models/src/secrets.rs`) strips JSON `null` values from the root object before deserialization. Previously a literal `"openai": null` / `"gemini": null` failed `serde_json::from_str` with `invalid type: null, expected a string`, which the bootstrap path swallowed and fell back to env-loading with a `None` seed.
- Subprocesses spawned with `SYSTEMPROMPT_SUBPROCESS=1` no longer rotate the manifest signing seed on each launch. `crates/domain/agent/src/services/agent_orchestration/process.rs` and `crates/domain/mcp/src/services/process/spawner.rs` propagate `MANIFEST_SIGNING_SECRET_SEED` from the parent's loaded `Secrets` into the spawn env. `secrets_bootstrap.rs::ensure_manifest_signing_seed` `bail!`s under `SYSTEMPROMPT_SUBPROCESS=1` with no seed in env.

### Security

- Manifest signatures use RFC 8785 (JCS) canonical JSON. Signer and verifier produce byte-identical canonical output.
- Cowork JWTs are minted with `audience: Cowork`, distinct from API tokens. Cross-audience misuse is rejected at validation.

### Removed

- **Breaking**: `DOMAIN_SEPARATOR` constant and the `Sha256(DOMAIN_SEPARATOR || jwt_secret)` derivation path in `crates/infra/security/src/manifest_signing.rs`.

### Internal

- `serde_jcs = "0.1"` added to `crates/infra/security/Cargo.toml`.
- Workspace `sha2` added to `crates/shared/models/Cargo.toml`.

## [0.4.0] - 2026-04-24

### Security

- **Fly.io cloud credentials now fail closed on API validation error** (`crates/infra/cloud/src/credentials_bootstrap.rs`). Previously, `CredentialsBootstrap::init()` demoted a validation error to `tracing::warn!` on Fly.io and continued with unvalidated credentials, so expired/revoked tokens only surfaced at the first downstream API call. Now propagates `CredentialsBootstrapError::ApiValidationFailed` unless the operator opts into fail-open behaviour with `SYSTEMPROMPT_ALLOW_UNVALIDATED_CREDS=1`. Non-Fly.io paths already failed closed and are unchanged.

- **Tarball extraction in `systemprompt-sync` hardened against path traversal** (`crates/app/sync/src/file_bundler.rs`). `extract_tarball` and `extract_tarball_selective` now reject symlinks and hard links, absolute paths, and any path containing `..`; enforce that the first path component is in the `INCLUDE_DIRS` allowlist (`agents`, `skills`, `content`, `web`, `config`, `profiles`, `plugins`, `hooks`); and canonicalise the destination parent, rejecting the entry if it escapes the target directory. Both entry points now funnel through a single `extract_tarball_filtered` helper. New `SyncError::TarballUnsafe(String)` variant. Pair with the equivalent hardening already in `crates/entry/api/src/routes/sync/files.rs`.

- **Auth middleware renamed to reflect its advisory role, `RequireAuth` extractor added** (`crates/entry/api/src/services/middleware/auth.rs`). `auth_middleware` → `auth_enrichment_middleware` and `AuthMiddleware::apply_auth_layer` → `apply_auth_enrichment_layer`. The middleware only attaches `Extension<AuthenticatedUser>` on successful JWT extraction and never rejects requests — enforcement lives in `ContextMiddleware`. New `RequireAuth(pub AuthenticatedUser)` extractor with `FromRequestParts` impl returns `401 Unauthorized` when the extension is absent, giving handlers a compile-time-checked auth primitive independent of `ContextMiddleware`. Neither the old function nor `apply_auth_layer` had external callers, so no downstream churn.

### Breaking

- **Removed `systemprompt::prelude::{Entity, EntityId, GenericRepository, RepositoryExt}`** (#5). The generic repository composed SQL at runtime from `E::TABLE`/`E::COLUMNS` and cannot satisfy the project's MANDATORY "SQLX macros only" rule (`query!` requires a string literal at compile time). No internal callers, no `impl Entity for` sites — the abstraction was dormant. Downstreams using the facade should migrate to per-entity repositories with `sqlx::query!()` / `query_as!()` (see `ServiceRepository`, `CleanupRepository` in `crates/infra/database/src/repository/` for the pattern). `crates/infra/database/src/repository/entity.rs` deleted.

- **`QueryExecutor::execute_query(sql, read_only)` replaced by `execute_readonly(sql, row_limit)` and `execute_write(sql)`** (#7). The old API passed a `bool` to switch modes; the new API encodes the mode in the entry point and returns the new `AdminSql` newtype's error variants if validation fails. Old callers using `executor.execute_query(sql, true)` become `executor.execute_readonly(sql, None)`; `executor.execute_query(sql, false)` becomes `executor.execute_write(sql)`.

### Changed

- **`DatabaseAdminService::{describe_table, get_table_indexes, count_rows}` now take `&SafeIdentifier` instead of `&str`** (#6). New `SafeIdentifier` newtype (exported from `systemprompt_database`) validates PostgreSQL identifiers at the boundary: 63-byte length cap, ASCII-letter-or-underscore lead, `[A-Za-z0-9_]` body only. Inline alphanumeric checks scattered across three admin methods removed; the invariant now rides the type. CLI callers (`db describe`, `db count`, `db indexes`) parse user input into a `SafeIdentifier` once at the CLI boundary and propagate it.

- **Admin SQL query executor hardened with `AdminSql` newtype and row cap** (#7). `AdminSql::parse_readonly(raw)` strips SQL line (`-- ...`) and block (`/* ... */`) comments, rejects multi-statement queries (any non-trailing `;`), requires a read-only prefix (`SELECT | WITH | EXPLAIN | SHOW | TABLE | VALUES`), and rejects forbidden keywords anywhere (drop, delete, insert, update, alter, create, truncate, grant, revoke, copy, vacuum, call, lock, set, reset, rename). Default row cap of 1000 on the read-only path, configurable per-call. Replaces the previous lowercase prefix + substring block-list, which missed comment-smuggled keywords and had no multi-statement guard.

### CI

- **New `ci/check-sqlx.sh` allowlist guard** (#8) fails if an unverified `sqlx::query*(...)` call appears outside a short list of structurally-dynamic sites (admin introspection, postgres driver, CLI bootstrap, integration test fixtures). Verified macros (`query!`, `query_as!`, `query_scalar!`) are unaffected. Wired into `just lint-sqlx` and `just style-check` step 4. Prevents regressions after this release tightens the unverified-query surface.

- **Regenerated per-crate `.sqlx/` offline caches** (#9) so `SQLX_OFFLINE=true cargo build --workspace` produces byte-identical output against the current live schema. Required for crates.io publishing.

## [0.3.2] - 2026-04-24

### Fixed

- **Static content route handler scoped slug lookup by `source_id`** (`crates/entry/api/src/services/static_content/static_files.rs`). `serve_static_content` extracted `(slug, source_id)` from the route matcher but discarded the source, then called `ContentRepository::get_by_slug(slug)` — a slug-only query. Any slug present in a different source (e.g. `about` as a page) caused `/guides/about`, `/documentation/about`, etc. to match a foreign record and return the "Content Not Prerendered" 500 page instead of 404. `source_id` is now threaded through `ContentPageRequest` and lookup uses `get_by_source_and_slug`.

- **Surface binary name and domain identifier on `Command::new` and `File::open` spawn errors across MCP, scheduler, sync, agent, and CLI paths.** The MCP port-manager reconciliation (`crates/domain/mcp/src/services/network/port_manager.rs`) shelled out to `lsof -ti :<port>` with bare `?` propagation. When `lsof` was missing from the runtime image, the ENOENT on `execve("lsof")` surfaced as a contextless `No such file or directory (os error 2)` and required `strace` to diagnose. Root fix is adding `lsof` to the runtime apt list, but the diagnosability gap is systemic: ~30% of `Command::new` sites discarded the binary name, args, and relevant identifier (port/pid/pattern/path) from the error path.

  Wrapped every flagged spawn site with `anyhow::Context::with_context` (or `tracing::warn!` where the return type is `Option`/`bool` and changing the signature would ripple through callers). Error messages now name the invocation (`failed to run \`lsof -ti :{port}\` for port {port}`) plus the domain identifier so operators don't have to re-derive context.

  Files touched: `crates/domain/mcp/src/services/network/port_manager.rs` (primary incident), `crates/domain/mcp/src/services/process/{pid_manager,cleanup,monitor,utils}.rs`, `crates/app/scheduler/src/services/orchestration/process_cleanup.rs` (previously silent `.ok()?` / `.is_ok_and(...)` converted to logging on failure), `crates/domain/agent/src/services/agent_orchestration/{port_manager,process}.rs`, `crates/domain/agent/src/services/agent_orchestration/orchestrator/cleanup.rs`, `crates/entry/cli/src/commands/cloud/tenant/docker/database.rs` (7 `docker exec psql` sites), `crates/entry/cli/src/shared/docker.rs`, `crates/app/sync/src/crate_deploy.rs` (new `SyncError::CommandSpawnFailed` variant), `crates/app/sync/src/file_bundler.rs` (new `SyncError::FileOpenFailed` variant), `crates/entry/cli/src/commands/web/templates/show.rs`.

- **HTTP-client timeout literals scattered across ~15 sites consolidated into `systemprompt_models::net`.** Generic 30s / 10s / 5s timeouts were inlined as `Duration::from_secs(…)` literals across cloud API, sync API, CIMD fetcher, OAuth credentials verify, CLI session auth, shared `SystempromptClient`, MCP streaming client, proxy client pool, API health checker, agent TCP monitor, and the two image-gen providers — with a dead `TimeoutConfiguration` struct in `crates/domain/agent/src/services/shared/resilience.rs` trying (and failing) to be the source of truth. Introduced `crates/shared/models/src/net.rs` with twelve named `Duration` consts (`HTTP_CONNECT_TIMEOUT`, `HTTP_DEFAULT_TIMEOUT`, `HTTP_HEALTH_CHECK_TIMEOUT`, `HTTP_AUTH_VERIFY_TIMEOUT`, `HTTP_SYNC_DEPLOY_TIMEOUT`, `HTTP_STREAM_CONNECT_TIMEOUT`, `HTTP_KEEPALIVE`, `HTTP_POOL_IDLE_TIMEOUT`, `AGENT_MONITOR_TCP_TIMEOUT`, `AGENT_READINESS_TCP_TIMEOUT`, `IMAGE_GEN_LONG_POLL_TIMEOUT`, `IMAGE_GEN_OPENAI_TIMEOUT`) so intent is explicit where values diverge (e.g. 300s for long-poll image gen, 2s for aggressive readiness probes, 15s for agent-startup grace). All 15 sites now reference these consts; every timeout preserves its previous numeric value — no runtime-behaviour change. Dead `TimeoutConfiguration` / `TimeoutType` enum deleted.

- **Consolidated further duplicate literals and removed an orphaned `AgentExtension` module.** `https://api.systemprompt.io` was inlined twice in `crates/infra/cloud/src/credentials_bootstrap.rs` (shadowing the existing `constants::api::PRODUCTION_URL`); both now reference the const. The A2A artifact-rendering extension URI `https://systemprompt.io/extensions/artifact-rendering/v1` was duplicated across 4 files — extracted to `systemprompt_models::a2a::ARTIFACT_RENDERING_URI` and wired into `agent_card.rs`, `artifact_transformer/mod.rs`, and `batch_builders.rs`. A second parallel `AgentExtension` struct in `crates/shared/models/src/a2a/agent_extension.rs` was an orphan (not in `mod.rs`, not referenced) — deleted. Production/sandbox DB hostnames (`db.systemprompt.io`, `db-sandbox.systemprompt.io`) in `swap_to_external_host` promoted to `constants::api::DB_PRODUCTION_HOST` / `DB_SANDBOX_HOST` next to the existing URL consts. `CALLBACK_TIMEOUT_SECS = 300` was declared twice (`oauth` and `checkout` modules) — lifted to a single top-level const aliased by both. User-Agent strings in the CIMD fetcher and webhook delivery service had hardcoded version suffixes (`systemprompt.io-OS/2.0`, `systemprompt.io-Webhook/1.0`) — now use `concat!("…/", env!("CARGO_PKG_VERSION"))` so the UA always matches the running binary.

- **CLI `--version` and API discovery reported stale hardcoded versions; protocol-spec versions were duplicated as literals.** `crates/entry/cli/src/args.rs:80` pinned the clap `#[command(version = "0.1.0")]` attribute to a literal, so `systemprompt --version` returned `0.1.0` regardless of the workspace version or the release tag. Swapped to `env!("CARGO_PKG_VERSION")` which clap resolves at build time from the crate's inherited workspace version. Same fix applied to the API gateway discovery endpoint (`crates/entry/api/src/services/server/discovery.rs:18`, user-visible `/` response) and the plugin marketplace generator (`crates/entry/cli/src/commands/core/plugins/generate/marketplace.rs:60`). Extracted the A2A and MCP protocol-spec versions into named constants to eliminate duplicate literals: `systemprompt_agent::A2A_PROTOCOL_VERSION = "0.3.0"` (replaces duplicates at `crates/domain/agent/src/models/web/card_input.rs:31` and `crates/entry/cli/src/commands/admin/agents/create.rs:94`) and `systemprompt_mcp::MCP_PROTOCOL_VERSION = "2024-11-05"` (replaces duplicates at `crates/domain/mcp/src/services/registry/trait_impl.rs:87` and `crates/entry/api/src/routes/agent/registry.rs:127`). These are pinned to external protocol specs — not our crate version — so the const form preserves intent while killing drift risk.

## [0.3.1] - 2026-04-22

### Fixed

- **Gateway tracing — six bugs overstating cost ~130× and hiding every downstream observability surface.** End-to-end audit (documented in `cowork-tracing.md`) of a live minimax gateway request found that cost reporting and every CLI read path (`audit --full`, `trace list`, `trace show`, `analytics conversations list`) was broken for gateway traffic. Root fixes in dependency order:

  - **`AnthropicCompatibleUpstream` honours `upstream_model`** (`crates/entry/api/src/services/gateway/upstream.rs`). Previously forwarded the raw request body unchanged, sending the client's `claude-sonnet-4-6` string to minimax regardless of `route.upstream_model`. Now computes `ctx.route.effective_upstream_model(&ctx.request.model)`, rewrites `body.model` only if it differs (pass-through stays zero-copy), and captures `response.model` into a new `UpstreamOutcome::Buffered { served_model, .. }` field so the audit layer learns what minimax actually served.

  - **`ai_requests.model` now stores the served model, not the client request** (`crates/entry/api/src/routes/gateway/messages.rs`, `crates/entry/api/src/services/gateway/audit.rs`). `GatewayRequestContext.model` is seeded from `route.effective_upstream_model()` at handler entry. `GatewayAudit::set_served_model()` overwrites `ai_requests.model` via new `AiRequestRepository::update_model` when the upstream response's `model` field differs from the route guess. Streaming path captures this from the `message_start` SSE frame via `stream_tap`.

  - **Real minimax pricing + unreachable match arm removed** (`crates/entry/api/src/services/gateway/pricing.rs`). The previous minimax branch had two identical `ModelPricing { 0.2, 1.1 }` arms (dead pattern match) at rates ~130× actual MiniMax API pricing. Replaced with per-family rates (`minimax-text-01` / `abab6.5` at $0.0002/$0.0011 per 1k, `minimax-m1` / `abab7-chat-preview` at $0.0004/$0.0022). Unknown models now fall through to `unknown()` which logs a warning and returns zero cost — missing entries are loud instead of silently overbilling. Pricing lookup moved from `GatewayAudit::new()` to `GatewayAudit::complete()` so the served model drives the rate.

  - **`ai_request_messages` populated from gateway path** (`crates/entry/api/src/services/gateway/audit.rs`, `crates/entry/api/src/services/gateway/parse.rs`). `GatewayAudit::open()` now parses the `AnthropicGatewayRequest` and inserts each message (plus any `system` prompt at `sequence_number=0`) via `AiRequestRepository::insert_message`. New `flatten_system_prompt` / `flatten_message_content` helpers join text blocks and JSON-encode tool_use / tool_result blocks. `complete()` appends the assistant response via `add_response_message`, extracted by new `parse::extract_assistant_text`. `audit <id> --full` now shows the full conversation turn instead of `"messages": []`.

  - **Gateway traces visible in `trace list` / `trace show`** (`crates/infra/logging/src/trace/list_queries.rs`). The `require_tracked` filter required `status IS NOT NULL`, which comes from `agent_tasks` — gateway requests don't create task rows, so their traces were hidden unless `--include-system` was passed. Filter dropped; `exclude_system` still drops the literal `"system"` bucket. `trace show` already renders AI summary when log events are empty, so it surfaces gateway traces as soon as they're discoverable.

  - **Gateway sessions in `analytics conversations list`** (`crates/domain/analytics/src/repository/conversations.rs`). `list_conversations` was `user_contexts`-only, populated exclusively by the agent path. Query rewritten as UNION of two CTEs: the original `agent_convs` (unchanged semantics) and a new `gateway_convs` that synthesizes rows from `ai_requests` where `task_id IS NULL`, grouped by `session_id`, counting `ai_request_messages` (populated by the Bug 3 fix). A `NOT EXISTS` guard prevents double-counting sessions that also have a `user_contexts` row.

  Added new `AiRequestRepository::update_model(id, model)` method (`crates/domain/ai/src/repository/ai_requests/mutations.rs`).

### Changed

- **Gateway helpers extracted to `gateway::flatten`** (`crates/entry/api/src/services/gateway/flatten.rs`, new). Consolidates `flatten_system_prompt`, `flatten_message_content`, `rewrite_request_model` (body JSON substitution for Anthropic-compatible upstream), and `parse_served_model` (response-body model extraction) into one module. Keeps `audit.rs` and `upstream.rs` near the 300-line coding-standards cap and isolates the JSON-at-protocol-boundary surface. Audit `build_record`, `persist_request_messages`, `persist_tool_calls` split into dedicated methods for function-length discipline.

  Verification: `cargo check --workspace` + `cargo clippy --workspace --all-targets` clean with `-D warnings`; `cargo fmt --all -- --check` clean; `systemprompt-api-tests` (429 passing) and `systemprompt-logging-tests` green. Expected end-to-end behavior: a minimax request now records cost within ±5% of the real MiniMax invoice, `audit --full` shows the full conversation, and the trace/analytics CLI commands surface gateway traffic without flags.

---

## [0.3.0] - 2026-04-22

### Added

- **LLM Gateway — `/v1/messages` inference routing.** Organisations using Claude for Work (formerly Claude Cowork) can now set `api_external_url` in their fleet MDM configuration to `https://systemprompt.io` and have every Claude Desktop inference request flow through the gateway. The gateway:
  - Exposes `POST /v1/messages` at the Anthropic wire format — fully compatible with the Claude API SDK, Claude Desktop, and any Anthropic-SDK client.
  - Authenticates with a systemprompt JWT carried in the `x-api-key` header (falls back to `Authorization: Bearer`). No additional API key is issued; the organisation's existing user JWTs serve as the credential.
  - Routes requests to any configured upstream provider based on `model_pattern` rules in the profile YAML. Supported provider types: `anthropic`, `openai` (OpenAI-compatible), `moonshot` (Kimi), `qwen`, `gemini` (stub — not yet dispatched).
  - **Anthropic upstream**: transparent byte proxy. Raw request bytes forwarded verbatim to the upstream endpoint with the upstream API key substituted; the response stream is piped back unmodified. Preserves extended thinking blocks, cache-control headers, and all Anthropic-specific SSE events exactly.
  - **OpenAI-compatible upstream**: converts Anthropic request format → OpenAI `/v1/chat/completions` format, proxies to the upstream, converts the response back to Anthropic format. For streaming, maps OpenAI SSE delta events to Anthropic `message_start` / `content_block_start` / `content_block_delta` / `message_delta` / `message_stop` SSE frames.
  - **API key resolution**: upstream API keys are resolved from the existing secrets file by secret name (`api_key_secret` in the route config). No new credential storage mechanism.
  - **Conditional mount**: the `/v1` router is only registered when `gateway.enabled: true` in the active profile — zero overhead for deployments that don't use the gateway.

- **Gateway profile configuration schema.** New `gateway` block in profile YAML (all fields optional; block absent = gateway disabled):

  ```yaml
  gateway:
    enabled: true
    routes:
      - model_pattern: "claude-*"
        provider: anthropic
        endpoint: "https://api.anthropic.com/v1"
        api_key_secret: "anthropic_api_key"
      - model_pattern: "moonshot-*"
        provider: moonshot
        endpoint: "https://api.moonshot.cn/v1"
        api_key_secret: "kimi_api_key"
        upstream_model: "moonshot-v1-8k"   # optional: override model name sent upstream
      - model_pattern: "qwen-*"
        provider: qwen
        endpoint: "https://dashscope.aliyuncs.com/compatible-mode/v1"
        api_key_secret: "qwen_api_key"
      - model_pattern: "*"                  # fallback route
        provider: anthropic
        endpoint: "https://api.anthropic.com/v1"
        api_key_secret: "anthropic_api_key"
  ```

  Routes are evaluated in order; first `model_pattern` match wins. Patterns support `*` wildcard prefix/suffix matching. `extra_headers` map is available per route for provider-specific requirements.

- **`GatewayProvider::is_openai_compatible()`** — `const fn` on the provider enum; returns `true` for `OpenAI`, `Moonshot`, `Qwen`. Used internally to select the conversion path.

- **`GatewayRoute::find_route(model)`** — resolves the first matching route for a given model name from a `GatewayConfig`. Returns `None` if no route matches (handler returns 404).

- **`GatewayRoute::effective_upstream_model(model)`** — returns `upstream_model` if set, otherwise echoes the client-provided model name. Enables transparent model aliasing (e.g. client requests `moonshot-v1-8k`; gateway can remap to a different upstream model name without the client knowing).

- **`JwtContextExtractor::extract_for_gateway(jwt_token: &JwtToken)`** — new method on the JWT middleware extractor. Accepts a typed `JwtToken` identifier (not a raw `&str`), validates it, and returns a `RequestContext`. Used by the gateway handler to validate the `x-api-key` credential without relying on the standard `Authorization: Bearer` middleware layer.

- **`ApiPaths::GATEWAY_BASE`** constant — `/v1` path prefix for the gateway router.

- **Cowork credential-helper auth path.** Claude for Work clients configure a `Credential helper script` that prints a bearer token on stdout; core now ships the helper binary plus the matching gateway endpoints that exchange a lower-privilege credential for a short-lived JWT carrying canonical identity headers.

  Gateway endpoints (mounted under `/v1/gateway/auth/cowork/` when `gateway.enabled: true`):

  - `POST /pat` — `Authorization: Bearer <pat>` → verifies the PAT via `systemprompt_users::ApiKeyService::verify`, loads the user via `systemprompt_oauth::repository::OAuthRepository::get_authenticated_user`, returns `{token, ttl, headers}` with a fresh JWT and the canonical header map.
  - `POST /session` — stub returning `501` (dashboard-cookie exchange not yet wired).
  - `POST /mtls` — stub returning `501` (device-cert exchange not yet wired).
  - `GET /capabilities` — returns `{"modes":["pat"]}`; probes advertise which exchange modes the deployment accepts.

  The JWT-assembly + header map live in `systemprompt_oauth::services::cowork` (`issue_cowork_access`, `issue_cowork_access_with`, `CoworkAuthResult`) so the route handler in `entry/api` stays thin — it only extracts the bearer, verifies via `ApiKeyService`, and calls the oauth-domain service. Headers returned in the response body use core's canonical constants from `systemprompt_identifiers::headers::*` (`x-user-id`, `x-session-id`, `x-trace-id`, `x-client-id`, `x-tenant-id`, `x-policy-version`, `x-call-source`) so Cowork merges them into every subsequent `/v1/messages` call and the gateway middleware reads real identity on every request.

- **`systemprompt-cowork` credential helper + sync agent binary.** Standalone crate at `bin/cowork/` (excluded from the workspace so it does not compile during `cargo build --workspace` and does not land in the `systemprompt` crates.io package). Dependency footprint is deliberately minimal (`ureq` + `rustls` + `serde` + `toml` + `ed25519-dalek`) — no `tokio`, `sqlx`, or `axum`.

  - **Progressive capability ladder**: probes credential providers in descending strength (mTLS → dashboard session → PAT). First provider that returns a token wins; absent providers return `NotConfigured` and the chain falls through. No user-facing "pick a mode" step.
  - **Providers** (`src/providers/{mtls,session,pat}.rs`) share a single `AuthProvider` trait returning `Result<HelperOutput, AuthError>` where `AuthError::NotConfigured` silently advances the chain.
  - **Config**: TOML at `~/.config/systemprompt/systemprompt-cowork.toml` (or `$SP_COWORK_CONFIG`). All sections optional — absent sections mean the provider is skipped. Dev overrides: `$SP_COWORK_GATEWAY_URL`, `$SP_COWORK_PAT`, `$SP_COWORK_DEVICE_CERT`, `$SP_COWORK_USER_ASSERTION`.
  - **Cache**: signed JWT + expiry written to the OS cache dir with mode `0600` on unix. Cached token is emitted directly if valid; only on cache miss does the probe chain run.
  - **Stdout contract**: exactly one JSON object matching `{token, ttl, headers}` — Anthropic's `inferenceCredentialHelper` format. All diagnostics go to stderr. Exit 0 on success, non-zero on failure.
  - **Sync agent**: `install`, `sync`, `validate`, `uninstall` manage Cowork's `org-plugins/` mount (macOS `/Library/Application Support/Claude/org-plugins/`, Windows `C:\ProgramData\Claude\org-plugins\`, Linux `${XDG_DATA_HOME:-$HOME/.local/share}/Claude/org-plugins/`) — pulling signed plugin manifests and managed MCP allowlists from the gateway.
  - **Release cadence**: tagged `cowork-v*`; `.github/workflows/cowork-release.yml` builds binaries for `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-pc-windows-msvc`, and `x86_64-unknown-linux-gnu`, attaches them to a GitHub Release with SHA256SUMS. Triggered only by the helper tag pattern; core's normal CI is untouched.
  - **Build targets**: `just build-cowork [target]` and `just build-cowork-all`.

- **`ClientId::cowork()`** constructor — returns `sp_cowork`, recognised as `ClientType::FirstParty` via the existing `sp_` prefix rule. Used by the Cowork JWT issuance path so every token issued to a Cowork session can be identified as first-party Cowork traffic in audit logs.

- **`SessionSource::Cowork`** variant + `SessionSource::from_client_id("sp_cowork") → Cowork`. Used as the `x-call-source` header value on Cowork-issued tokens so downstream middleware and analytics can distinguish Cowork sessions from Web / CLI / API / OAuth / MCP sessions.

- **`systemprompt_identifiers::PolicyVersion`** — new typed ID with `PolicyVersion::unversioned()` constructor. Exposed in the Cowork helper's header response as `x-policy-version` so a future policy-bundle-hash propagation feature plugs in without changing the wire contract.

- **`systemprompt_identifiers::headers::TENANT_ID` / `POLICY_VERSION`** — new canonical header constants (`x-tenant-id`, `x-policy-version`) alongside the existing `USER_ID`, `SESSION_ID`, `TRACE_ID`, `CLIENT_ID` family. All Cowork-issued tokens carry the full set in the response body's `headers` map.

- **Gateway provider registry — extensions can register custom upstreams.** `GatewayProvider` is no longer a closed enum; `GatewayRoute.provider` is now a free-form string tag resolved at dispatch time against a registry built at startup. Extension crates register new providers with:

  ```rust
  inventory::submit! {
      systemprompt_api::services::gateway::GatewayUpstreamRegistration {
          tag: "my-provider",
          factory: || std::sync::Arc::new(MyUpstream),
      }
  }
  ```

  The new `GatewayUpstream` trait (`async fn proxy(&self, ctx: UpstreamCtx<'_>)`) is the single integration seam. Built-in tags seeded automatically: `anthropic`, `minimax`, `openai`, `moonshot`, `qwen`. Extension-registered tags may shadow built-ins (logged as a warning).

- **MiniMax provider.** MiniMax ships an Anthropic-compatible endpoint at `https://api.minimax.io/anthropic`, so the new `minimax` tag reuses the Anthropic-compatible upstream verbatim — streaming, tool use, and `thinking` blocks pass through untouched. Example route:

  ```yaml
  gateway:
    enabled: true
    routes:
      - model_pattern: "MiniMax-*"
        provider: minimax
        endpoint: https://api.minimax.io/anthropic
        api_key_secret: minimax
  ```

  The `api_key_secret` resolves through `Secrets.custom`, so no changes to the secrets schema are required.

- **Gateway governance — full audit, policy, quota, and safety pipeline.** Every `/v1/messages` call now lands a structured audit trail, enforces tenant-scoped policy, and runs through a pluggable safety scanner before and after dispatch. This closes the product-level gap where the gateway proxied requests to MiniMax/Anthropic/OpenAI upstreams but persisted nothing beyond a one-line tracing log. For a platform whose core promise is "governance for all AI calls", this is the spine that makes the promise enforceable rather than aspirational.

  - **`ai_requests` persistence on the gateway path.** The handler mints a typed `AiRequestId` at ingress, writes a `pending` row before dispatch (with `user_id`, `tenant_id`, `session_id`, `trace_id`, `provider`, `model`, `max_tokens`, `is_streaming`), and updates it to `completed` with `input_tokens` / `output_tokens` / `cost_microdollars` / `latency_ms` once the upstream response resolves. Non-streaming responses parse the buffered JSON to extract usage + `tool_use` blocks; streaming responses run through an SSE tap (see below) that captures the same data without mutating the byte stream. On upstream error, the row flips to `failed` with `error_message` populated. Audit writes are best-effort — a DB outage logs an ERROR but never blocks the proxied request.

  - **`ai_request_payloads` table — full request/response retention.** New JSONB columns per `AiRequestId`: `request_body`, `response_body`, plus truncation flags + byte counts. 256 KB cap per side; overflow writes `NULL` for the body and a head+tail excerpt (`request_excerpt` / `response_excerpt`, 8 KB each side with a `...<truncated N bytes>...` marker). Response capture for streams reconstructs the full byte payload from the tap before persisting. Payload writes are fire-and-forget (`tokio::spawn`) so the client connection closes at upstream speed regardless of DB write latency.

  - **`ai_request_tool_calls` — `tool_use` capture + `tool_result_payload` column.** Every `tool_use` block in the response (Anthropic `content[].type == "tool_use"` for buffered JSON; `content_block_start` + `input_json_delta` accumulation for SSE) writes one row to `ai_request_tool_calls` with sequence number, `ai_tool_call_id`, `tool_name`, and `tool_input` (64 KB cap with truncation marker). New nullable `tool_result_payload JSONB` column is added to close the loop on follow-up turns — the migration is in place; the match-on-`ai_tool_call_id` upsert from the next request is plumbed for a follow-up iteration.

  - **`ai_safety_findings` table + pluggable `SafetyScanner` trait.** New async trait at `crates/entry/api/src/services/gateway/safety/` with two implementations: `HeuristicScanner` (known jailbreak prefixes → severity=medium; email regex → low; Luhn-valid 16-digit credit card → high) and `NullScanner` (for tests). Scanning runs pre-dispatch on the request and post-dispatch on the response (per-chunk SSE scanning is wired but currently reuses the final-buffered path). Findings persist with phase (`request` / `response`), severity, category, and an excerpt. Current release is warn-only — findings land in the table and can be queried, but don't short-circuit the request. The policy `safety.block_categories` field is plumbed to the dispatch path and gates a `451` short-circuit in the next iteration.

  - **`ai_quota_buckets` table + token-bucket enforcement.** Per-`(tenant_id, user_id, window_seconds, window_start)` atomic counters via `INSERT ... ON CONFLICT DO UPDATE RETURNING` — Postgres serialises contention with no application-level lock. Pre-dispatch reserves 1 request; if any configured window exceeds its hard limit, dispatch returns `429 Too Many Requests` with a `Retry-After` header and the audit row flips to `failed` with `status_code='denied_quota'`. Post-dispatch, a second update adds `input_tokens` + `output_tokens` to the same buckets. Multiple windows (e.g. 60s / 3600s / 86400s) evaluate in order; first exceeded window wins.

  - **`ai_gateway_policies` table + `PolicyResolver`.** Tenant-scoped JSONB policies composed at dispatch: `allowed_models` (list of model names — anything else returns `403 Forbidden` with audit row `status='failed'`), `max_input_tokens_per_call`, `max_tool_depth`, `quota_windows`, and `safety` (scanner list + block categories). Resolution order: tenant-specific → global (`tenant_id IS NULL`) → compiled-in `GatewayPolicySpec::permissive()` fallback. 60-second in-memory TTL cache; DB unavailability logs a warning and returns the permissive fallback rather than wedging the gateway.

  - **SSE stream tap.** `crates/entry/api/src/services/gateway/stream_tap.rs` wraps the upstream `Stream<Item = Result<Bytes, io::Error>>` and re-emits every chunk to the client byte-identical, while parsing `message_start` / `message_delta` / `content_block_start` / `content_block_delta` / `content_block_stop` frames to accumulate usage + assemble `tool_use` blocks from `input_json_delta` fragments. On end-of-stream, `tokio::spawn` fires `audit.complete(usage, tool_calls, reconstructed_body)`; on upstream error, fires `audit.fail(error)`. The tap never mutates the proxied byte stream — clients that expect byte-exact Anthropic SSE get byte-exact Anthropic SSE.

  - **`x-systemprompt-request-id` response header.** Every gateway response (success, 403 policy denial, 429 quota denial, 451 safety denial, 500 upstream error) carries the minted `AiRequestId` as `x-systemprompt-request-id: <uuid>` so Cowork and any SDK caller can grep logs or the audit table by the same key. Header is also propagated into tracing spans.

  - **Pricing table.** `crates/entry/api/src/services/gateway/pricing.rs` resolves `(provider, model) → ModelPricing { input_cost_per_1k, output_cost_per_1k }` for the Claude 4.x family (Opus / Sonnet / Haiku), MiniMax-* (flat pricing), and GPT-4o family. Unknown pairs log a `WARN` and record `cost_microdollars=0` rather than failing the request, so an operator sees the gap in logs and adds the entry without an incident. Cost computation copies the proven formula from `crates/domain/ai/src/services/core/ai_service/stream_wrapper.rs` (`(input_tokens/1000 × input_cost + output_tokens/1000 × output_cost) × 1_000_000`).

  - **New typed IDs** in `systemprompt_identifiers`: `AiSafetyFindingId`, `AiQuotaBucketId`, `AiGatewayPolicyId` — all generated (UUID-backed) with the `schema` variant for OpenAPI exposure.

  - **New domain repositories** in `systemprompt_ai`: `AiRequestPayloadRepository`, `AiSafetyFindingRepository`, `AiQuotaBucketRepository`, `AiGatewayPolicyRepository`. `AiRequestRepository::insert_with_id(id, record)` is a new public method that lets the gateway audit own ID minting at ingress (the existing `insert(record)` still exists and generates a fresh ID for internal AI-service callers).

  - **`AiRequestRecord.tenant_id: Option<TenantId>`** — new field on the write model + matching `tenant_id()` setter on `AiRequestRecordBuilder`. The underlying `ai_requests` table gained `tenant_id VARCHAR(255)` via migration `001_gateway_governance.sql` with `(tenant_id)` and `(tenant_id, created_at)` indices.

  - **`JwtContextExtractor`-driven user attribution.** The gateway handler extracts `UserId`, `SessionId`, and `TraceId` from the validated JWT context (JWT path) or from the matched `ApiKeyRecord` (API key path), and reads optional `x-tenant-id` from request headers. An `AuthedPrincipal` struct bundles these four fields into a single `GatewayRequestContext` that every downstream module (audit, quota, policy, safety) reads. Previously `JwtContextExtractor::extract_for_gateway` validated the token but its result was discarded.

  - **New dependency edge**: `systemprompt-api` now depends on `systemprompt-ai` for repository access. The gateway service module gained seven new files (`audit.rs`, `parse.rs`, `pricing.rs`, `policy.rs`, `quota.rs`, `stream_tap.rs`, `safety/{mod,heuristic,null}.rs`) and `upstream.rs` was refactored to return a typed `UpstreamOutcome` enum (`Buffered { status, content_type, body } | Streaming { status, stream }`) instead of a raw `Response<Body>`, so the service layer can intercept for audit + policy enforcement before final response assembly.

### Changed

- **Gateway dispatch rewritten around the registry.** `GatewayService::dispatch` is now a thin shim: resolve route → resolve API key → look up the registered upstream → hand off to `upstream.proxy(ctx)`. The old hard-coded `match route.provider { ... }` is gone. The `GatewayProvider` enum (and its `is_openai_compatible()` / `as_str()` methods) have been removed; `GatewayRoute.provider` is a `String`. Anthropic-passthrough and OpenAI-compatible behaviours are preserved — their bodies were moved verbatim into `AnthropicCompatibleUpstream` and `OpenAiCompatibleUpstream` in the new `upstream.rs`. Unknown provider tags now fail fast with `Gateway provider 'xxx' is not registered`.

- **Analytics: broader conversion events + UTM expansion.** `event_data` column on `analytics_events` changed to `JSONB` (was `TEXT`) to support structured payload inspection. Added `utm_content` and `utm_term` UTM parameter columns to complete the full UTM dimension set. Conversion event definitions broadened to cover a wider range of funnel actions (subscription starts, trial activations, feature adoptions).

### Included from 0.2.5

- Workspace-wide Rust-standards sweep (see [0.2.5] entry below for full detail): zero inline comments, zero `unwrap_or_default()`, annotated `serde_json::Value` protocol boundaries, regenerated SQLx offline cache.

---

## [0.2.5] - 2026-04-20

### Changed
- **Workspace-wide Rust-standards sweep.** Executed a full audit against `instructions/prompt/rust.md` and the `rust-coding-standards` skill across `crates/{shared,infra,domain,app,entry}/**/src/`. Five parallel layer agents fixed every zero-tolerance violation they found; a final pass closed the clippy-exposed stragglers. `cargo clippy --workspace --all-targets -- -D warnings` now passes clean, `cargo fmt --all -- --check` is clean, `cargo build --workspace` succeeds. Changes:
  - **Deleted** `crates/shared/models/src/validation_report.rs` — dead 9-line backward-compat re-export, not declared in `lib.rs`, zero importers (all call sites already used `systemprompt_traits::validation_report` directly).
  - **Replaced every `unwrap_or_default()` in src code** (13 occurrences across 7 files). Fixes range from propagating a `Result` (`MarkdownResponse::to_markdown()` now returns `Result<String, serde_yaml::Error>`; its `IntoResponse` impl logs + returns 500 on failure) to idiomatic combinators (`map_or_else(Vec::new, Clone::clone)` in oauth/agent repositories) to explicit `if let Ok(...)` env-var inheritance in agent subprocess spawn. The schema sanitizer's `.next().unwrap_or_default()` became a proper `if let Some(Value::Object(inner))` after an invariant check.
  - **Deleted 19 inline `//` comments** across infra/cloud (4), domain/{ai,agent,analytics,oauth} (14), and entry/cli (15). Per rust.md §3, code documents itself through naming; the only retained `//` annotations are the `// JSON: …` markers on `serde_json::Value` protocol-boundary sites (explicit exception per the `rust-coding-standards` skill).
  - **Annotated ~82 `serde_json::Value` sites in infra** as protocol/schemaless boundaries (A2A JSON-RPC, MCP schemas, webhook payloads, dynamic DB admin queries, log visitors, JSON-Schema trees). Triage reports for all five layers written to `reports/audit/{shared,infra,domain,app,entry}-json-triage.md` (gitignored) with counts of Keep+annotate / Refactor / Defer categories; ~24 refactorable sites and ~106 deferred (API-surface) sites enumerated there for follow-up PRs.
- **Regenerated workspace `.sqlx` offline cache.** Commit `a55b1570e` (analytics conversion + utm) added `utm_content`, `utm_term`, and `event_data` columns to the live DB but the workspace-level sqlx query cache was not regenerated, so `cargo check -p systemprompt-analytics` failed with `SQLX_OFFLINE=true`. Cache now reflects current schema; analytics crate compiles clean again.

### Fixed
- `MarkdownResponse::to_markdown()` signature changed from `fn(&self) -> String` to `fn(&self) -> Result<String, serde_yaml::Error>`. The previous version silently swallowed frontmatter serialization failures via `unwrap_or_default()` and produced a response with no frontmatter. Callers now see the error or (at the HTTP boundary) a logged 500. Breaking for any external consumer of `MarkdownResponse::to_markdown()`; there are none in this repository.

### Audit
- Post-sweep verification greps confirm **zero** occurrences of `.unwrap()`, `unwrap_or_default()`, `panic!`, `todo!`, `unimplemented!`, `unsafe`, `///` doc comments, and `TODO|FIXME|HACK` in any non-test `src/` file across the workspace. `println!`/`eprintln!` retained only at legitimate CLI-output boundaries and in the `config/schema_validation` build-script helper (already guarded with `#[allow(clippy::print_stderr, clippy::print_stdout)]`).

## [0.2.4] - 2026-04-20

### Fixed
- **`admin agents registry` now defaults to the active profile's `api_external_url`.** Previously the command hard-coded `http://localhost:8080` as its gateway URL, so `systemprompt admin agents registry` failed with `Connection refused` on any profile that used a non-default port (e.g. `just setup-local ... 8081 5434`). The hint string on `--url` still advertised "default: http://localhost:8080" even after a user pointed a profile at a different host. Fix: read the active `ProfileBootstrap::get().server.api_external_url` first; fall back to `http://localhost:8080` only if no profile is loaded. `--url` still overrides both.

## [0.2.3] - 2026-04-20

### Fixed
- **Drop cloud-auth requirement for local-trial CLI sessions.** On a fresh template clone with `just setup-local`, the CLI gated a wide set of local-capable operations (`admin agents tools`, `plugins mcp tools/call`, `core contexts list`, trace lookups) behind `Cloud authentication required. Run 'systemprompt cloud auth login' to authenticate.`. Root cause: `SessionKey::from_tenant_id(Some("local_dev"))` returns `SessionKey::Tenant(...)`, not `SessionKey::Local`, so the `session_key.is_local()` branch in `create_new_session` was skipped and `CredentialsBootstrap::require()` fired. `resolve_local_user_email` had the same behavior inside the local-session branch when `session_email_hint` was absent. Fix: centralise the "is this a local-trial profile?" rule on `CloudConfig::is_local_trial()` / `Profile::is_local_trial()` (no `cloud` block, `tenant_id` starts with `local_`, or `validation ∈ {Warn, Skip}`); `create_new_session` now also treats local-trial profiles as local; `resolve_local_user_email` falls back to `admin@localhost.dev` — matching the address `demo/00-preflight.sh` uses, so CLI- and demo-created admin sessions share a user row. Genuine cloud entrypoints (`cloud sync`, `cloud tenant select`, `admin session login`, `admin session switch`) are unchanged and still require cloud credentials. `bootstrap.rs`' duplicated 12-line local-profile predicate now delegates to the shared helper.

## [0.2.2] - 2026-04-17

### Fixed
- **macOS build fix — `statvfs` type mismatch in health endpoint.** `get_disk_usage()` in `systemprompt-api` failed to compile on macOS (Darwin) because `nix::sys::statvfs` returns `u32` for `blocks()`, `blocks_available()`, and `blocks_free()` on macOS but `u64` on Linux, while `fragment_size()` returns platform-varying types. The `saturating_mul` calls required matching types. Fix: explicit `u64::from()` casts on all `statvfs` field accesses so the arithmetic is platform-independent.

### Changed
- Docs sweep: refreshed READMEs across all 30 crates to align with the 0.2.x naming and current feature matrix.
- Relocated generator asset/build/markdown/sitemap unit tests out of `crates/app/generator/tests/` into the dedicated test workspace at `crates/tests/unit/app/generator/src/` to match the "test crates live outside the main workspace" rule. Added missing `unit_tests` module to the scheduler test workspace.

## [0.2.1] - 2026-04-16

### Fixed
- **Idempotent agent migrations — fix startup crash on existing databases.** Migrations `003_a2a_v1_task_states.sql` and `004_ai_requests_task_fk.sql` could brick service startup on sites with pre-existing data. Root cause: `SqlExecutor::execute_statements_parsed` splits SQL on semicolons and runs each statement as a separate `execute_raw` call against the connection pool, so the `BEGIN`/`COMMIT` wrapper in migration 003 was a no-op (each statement auto-committed on potentially different connections). If any statement succeeded but the migration recording failed, the next startup retried the migration and hit already-applied DDL. Three fixes: (1) removed the ineffective `BEGIN`/`COMMIT` from migration 003, (2) added missing `UPDATE` for `'pending'` → `'TASK_STATE_PENDING'` status value that would cause the CHECK constraint to reject existing rows, (3) wrapped the `ADD CONSTRAINT` in migration 004 with an `IF NOT EXISTS` guard via a `DO` block so re-running the migration after a partial failure is safe.
- **Gemini schema sanitizer — nullable & $ref handling.** `ProviderCapabilities::gemini()` now reports `features.references = false` and `features.definitions = false`, so the sanitizer strips `$ref` / `$defs` / `definitions` before the request reaches Gemini. Gemini's `FunctionDeclaration.parameters` uses `google.api.JsonSchema`, which rejects those keywords with `400 INVALID_ARGUMENT`.
- **Nullable normalisation in `SchemaSanitizer`.** New `normalize_nullable` pre-pass rewrites both JSON-Schema nullable forms into Gemini/OpenAPI `nullable: true`: `{"type": ["string", "null"]}` collapses to `{"type": "string", "nullable": true}`, and `{"anyOf": [{"type": "X"}, {"type": "null"}]}` collapses to `{"type": "X", "nullable": true}`. Non-null `anyOf` unions and `type` arrays without a `"null"` sibling are left untouched. Runs before composition stripping so the result survives the rest of the pipeline.
- **Analytics — per-agent cost breakdown reconciles with totals.** `CostAnalyticsRepository::get_breakdown_by_agent` now returns an always-present `'unattributed'` aggregate row alongside the top-N attributed agents, via a `UNION ALL` of (INNER JOIN'd attributed spend) + (unattributed spend with `task_id IS NULL OR agent_name IS NULL`). The invariant `sum(breakdown_by_agent.cost) == get_summary().total_cost` now holds for every window. An in-flight edit had switched to a plain `INNER JOIN`, silently dropping ad-hoc / context-less AI spend from the governance audit — exactly the shadow-AI blindspot the report exists to surface. `LIMIT` only bounds the attributed top-N; the unattributed row is never truncated. Four new reconciliation tests in `crates/tests/unit/domain/analytics/src/repository/costs.rs` lock the invariant in place (all-attributed, mixed-null, limit-survival, empty-window).
- **Agent extension — registered unreleased `003_a2a_v1_task_states.sql` migration.** Found during this release: `crates/domain/agent/schema/migrations/003_a2a_v1_task_states.sql` was added during the 0.1.22 A2A v1 protocol upgrade but never registered in `AgentExtension::migrations()`, so the live UPDATE that rewrites legacy `submitted`/`working`/... rows to `TASK_STATE_*` SCREAMING_SNAKE_CASE and tightens the CHECK constraint had never run on any deployed instance. Any database with pre-0.1.22 task rows would have been in an inconsistent state. Migration is now wired up and runs on next migration sweep.

### Schema
- **`ai_requests.task_id` is now a proper FK to `agent_tasks(task_id)`.** New migration `crates/domain/agent/schema/migrations/004_ai_requests_task_fk.sql` normalises the column type from `VARCHAR(255)` to `TEXT` (matches parent PK), nulls out pre-existing orphaned references (preserving cost/token data), and installs `FOREIGN KEY (task_id) REFERENCES agent_tasks(task_id) ON DELETE SET NULL`. From here on, orphaned `task_id` values are structurally impossible, and deleting an agent task rolls its historical AI spend up under `'unattributed'` in the cost breakdown rather than cascading away audit data. `systemprompt-agent` now declares `"ai"` as an explicit extension dependency so the migration runs after the `ai_requests` table exists. Migration placement rationale: ai (weight 35) loads before agent (40), so a cross-domain FK from `ai_requests → agent_tasks` must be installed from the agent side.

### Removed — Dead `CreateAiRequest` insert path
- Deleted `CreateAiRequest` struct and `AiRequestRepository::create()` method from `crates/domain/ai/src/repository/ai_requests/`, plus associated re-exports in `crates/domain/ai/src/lib.rs`, `repository/mod.rs`, and `ai_requests/mod.rs`. The struct had no `task_id` field and no production callers; its existence invited a future bug where a new caller would use it and produce unattributable AI spend rows. The live insert path remains `AiRequestRecord` + `AiRequestRepository::insert()`, which already carries `task_id: Option<TaskId>`. BREAKING for any external crate importing `CreateAiRequest`; there are none in this repository.

### Chores
- Workspace bumped to 0.2.1; per-crate descriptions swept (b5b13d59c).
- **Cargo feature-flag sweep.** Removed unused / always-on feature gates across the workspace: `systemprompt-extension` (`web`, `plugin-discovery`), `systemprompt-logging` (empty `web`), `systemprompt-database` (`api` + dead optional `axum`), `systemprompt-mcp` (empty `cli`), `systemprompt-oauth` (`web`), `systemprompt-agent` (`web`, empty `cli`), `systemprompt-analytics` (`web`), `systemprompt-scheduler` (empty block), `systemprompt-cloud` (empty `test-utils`). Inlined previously-optional deps (axum, tower, tower-http, bytes, jsonwebtoken, tokio-stream, urlencoding) and stripped ~40 `#[cfg(feature = ...)]` gates. Legitimate gates kept: `models/web`, `traits/web`, `identifiers/sqlx`, `template-provider/tokio`, `logging/cli`, `runtime/geolocation`, `analytics/geolocation`, `generator/image-processing`, and the facade crate's user-facing feature matrix.

### Services Config Migration (Phases 1-4)

A workspace-wide breaking change to the services configuration layer.

- **Phase 1 — Schema**: `ServicesConfig` grew first-class `skills` and `content` fields; `PluginConfig` gained `content_sources` bindings; both `ServicesConfig` and `PartialServicesConfig` are locked with `#[serde(deny_unknown_fields)]`; `ServicesConfig::validate()` now enforces plugin bindings and skill map-key integrity.
- **Phase 2 — WebConfig**: deleted the 3-field stub `WebConfig` in `systemprompt-models` and switched `ServicesConfig.web` to `Option<systemprompt_provider_contracts::WebConfig>` so the rich branding/colors/typography/layout config round-trips through the loader. Breaking for any caller constructing the stub directly.
- **Phase 3 — Loader**: `ConfigLoader` is now the single loader with recursive `includes:` resolution and cycle detection. Removed `EnhancedConfigLoader`, `IncludeResolver`, `ConfigLoader::discover_and_load_agents`, and `ConfigWriter::add_include`. Loading is now pure — no auto-discovery side effects on `config.yaml`. Users must list every include explicitly.
- **Phase 4 — Callers**: `cloud profile show` and all remaining call sites migrated to `ConfigLoader::load()`.

### Phase 5 — Typed-ID migration (trait surfaces + DTOs)

- Migrated `ContextProvider`, `UserProvider`, `RoleProvider` trait surfaces from raw `&str` to typed identifiers (`UserId`, `ContextId`, `SessionId`). Breaking for any external impl.
- Waves 1–5 (commits 13568bcfa…806cc2844) covered canonical models, A2A protocol, oauth/webauthn, AI rows, tracing, app sync/generator, and CLI residuals.
- DTO sweep: migrated the remaining raw `String` ID fields across cloud DTOs, services models, AI rows, analytics events, A2A protocol messages, and API/CLI surfaces to `systemprompt_identifiers` typed IDs. Serialization is unchanged (typed IDs round-trip as plain strings).
- Wave 7 — **completed**: all 69 remaining raw `String` ID fields across shared traits, shared models, infra (security claims), domain (users, analytics, ai, oauth, agent), app/sync, entry/api webauthn+anonymous+proxy, and entry/cli plugins/content/logs migrated to typed identifiers. `LogId` gained `JsonSchema` support. `WebAuthnService::finish_registration_with_token` and `WebAuthnService::finish_registration` now return `UserId` instead of `String`. Vendor/external IDs (WebAuthn FIDO2 credentials, A2A third-party agent-card skill IDs, third-party webhook endpoint IDs, external LLM model names, CTA button action identifiers) kept as `String` with `// JSON:` justification comments per the narrow exception in CLAUDE.md. Clap CLI arguments that accept user-provided partial lookups (`ShowArgs.id`, `AuditArgs.id`, etc.) annotated with `// CLI:` and kept as `String` by design.

### Removed — Dead authorization stubs

- Deleted `crates/domain/oauth/src/services/auth_provider.rs` in its entirety. `JwtAuthProvider`, `JwtAuthorizationProvider`, and `TraitBasedAuthService` were dead since v0.0.1: zero production callers, and `JwtAuthorizationProvider::{authorize, get_permissions}` silently returned `Ok(true)` / `Ok(vec![])` regardless of input — a latent authorization footgun. Real permission logic continues to live in `JwtClaims::get_permissions()` and `crates/domain/mcp/src/middleware/rbac.rs`.
- Collapsed the `AuthorizationProvider` trait and `AuthProvider` trait entirely — both were single-impl traits with no call sites. Removed associated dead types: `AuthAction`, `AuthPermission`, `TokenPair`, `TokenClaims`, `DynAuthProvider`, `DynAuthorizationProvider`. BREAKING for any external crate importing these names; there are none in this repository.
- Removed `JwtAuthProvider::{refresh_token, revoke_token}` which returned `"not yet implemented"` errors and had zero callers. The real OAuth refresh/revoke lifecycle uses `OAuthRepository` and the token endpoints — unaffected.

### Fixed

- Zero-warning, zero-error build across workspace (`cargo build --workspace` + `cargo clippy --workspace --all-targets -- -D warnings` + `cargo fmt --all --check`).
- Resolved clippy `needless_borrow` in `crates/entry/api/src/routes/oauth/endpoints/anonymous.rs` and `.../token/generation.rs`.
- Resolved clippy `useless_conversion` and `single_match_else` in `crates/entry/cli/src/commands/admin/agents/message.rs` and `.../cloud/sync/admin_user/sync.rs`.
- Dropped unused parameters in `AgentOrchestrationDatabase::{mark_failed, get_unresponsive_agents}`, `MonitorService::cleanup_unresponsive_agents`, and `a2a_server::handlers::request::validation::should_require_oauth` — signatures no longer lie about what the implementation uses.
- Removed 15 forbidden doc comments from `crates/shared/models/src/macros.rs` (standards: no `///` in production code).
- Removed 1 unnecessary path qualification in `crates/domain/agent/src/services/a2a_server/auth/validation.rs`.

## [0.1.22] - 2026-04-07

### Changed
- **A2A Protocol v1.0.0 Migration** — upgrade from v0.3.0 to the first stable release (Linux Foundation, March 2026)
  - TaskState: kebab-case to `TASK_STATE_*` SCREAMING_SNAKE_CASE (`"submitted"` -> `"TASK_STATE_SUBMITTED"`)
  - MessageRole: `"user"`/`"agent"` to `"ROLE_USER"`/`"ROLE_AGENT"`, now a typed enum
  - Part: tagged enum (`kind` discriminator) to untagged (field-presence discrimination)
  - FileWithBytes renamed to FileContent; `bytes` now optional, added `url` field for URL-referenced files
  - Message: removed `kind` field, `id` renamed to `message_id`
  - Task: removed `kind` field, added `created_at`/`last_modified` timestamps
  - Artifact: `name` renamed to `title`
  - AgentCard: collapsed `url`/`preferred_transport`/`additional_interfaces` into `supported_interfaces` array with per-interface protocol version
  - TransportProtocol renamed to ProtocolBinding (type alias kept)
  - JSON-RPC methods: PascalCase (`"message/send"` -> `"SendMessage"`, `"tasks/get"` -> `"GetTask"`, etc.)

### Fixed
- Resolve all build warnings and clippy errors across workspace
  - Add missing `Debug` derives on `BuildMetadataParams`, `HtmlBuilder`, `TokenGenerationParams`, `AuthCodeValidationParams`
  - Fix ambiguous glob re-export of `validation` module in OAuth endpoints
  - Allow `struct_field_names` on A2A `Message` (protocol-required field name)
  - Replace redundant closures with function references in agent URL extraction
  - Add `const fn` to `TaskState::is_terminal()`, `can_transition_to()`, and `role_to_str()`
  - Use `Self` instead of concrete type in `TaskState::can_transition_to()` parameter

### Added
- Database migration `003_a2a_v1_task_states.sql` for task status value migration
- TaskState `is_terminal()` and `can_transition_to()` methods for state machine validation
- Backward-compatible task state parsing (accepts both old and new format strings)

## [0.1.21] - 2026-04-01

### Fixed
- Remove silent error swallowing in `DatabaseLayer::flush()` — all DB log write failures are now reported with entry count
- Logging initialization order: `init_logging(db_pool)` now works regardless of whether `init_console_logging()` was called first

### Changed
- Replace `DatabaseLayer` with `ProxyDatabaseLayer` architecture — subscriber is always initialized with a proxy that accepts a DB pool attachment at any time
- Move `AppContext` construction logic from `new_internal()` into `AppContextBuilder::build()` — builder owns its construction
- Move `init_logging()` call earlier in `AppContextBuilder::build()` — immediately after DB pool creation, before extension discovery
- Extract `AppContextBuilder` into `crates/app/runtime/src/builder.rs`
- Extract `ProxyDatabaseLayer` and shared span/event helpers into `crates/infra/logging/src/layer/proxy.rs`
- Remove redundant `init_logging()` call from `serve.rs`

## [0.1.20] - 2026-04-01

### Changed
- Upgrade `rmcp`/`rmcp-macros` from 1.1 to 1.3
- Simplify MCP `StreamableHttpServerConfig` to use library defaults instead of manual field construction
- Adapt MCP HTTP client to rmcp 1.3 API: replace removed `AuthRequiredError` with `UnexpectedServerResponse`
- Rebrand README messaging: reposition from "production infrastructure for AI agents" to "AI governance layer" with compliance-first positioning (SOC 2, ISO 27001, HIPAA, FedRAMP)
- Update README navigation: "Playbooks" → "Skills"

### Added
- `ensure_project_scaffolding()` function in cloud init — auto-creates `services/` and `web/` directories during local tenant setup
- Project scaffolding step integrated into local tenant creation workflow (runs before profile setup)

### Refactored
- Resolve all remaining clippy errors and warnings to achieve zero-warning build
- Introduce parameter structs for `too_many_arguments` in agent services (Wave 2)
- Eliminate all redundant closure violations (Wave 1)
- Split large files: complete `deploy/mod.rs` split and file split extractions from source files
- Remove `unsafe` blocks and convert static SQL to compile-time verified macros

### Removed
- Clean up ~120 stale SQLx query cache files from sync crate

## [0.1.19] - 2026-03-31

### Added
- `CloudEnterpriseLicenseInfo` struct for domain-based enterprise licensing
- `enterprise` field on `UserMeResponse` (optional, backward-compatible)
- `EnterpriseLicenseInfo` type alias
- Structured streaming with `StreamChunk` enum for typed AI provider responses with token usage tracking
- Pricing-based cost calculation for streaming responses
- Authenticated `/api/v1/health/detail` endpoint with full system diagnostics (split from public health check)
- Email validation module (`validation.rs`) with shared `is_valid_email` helper
- ConnectInfo fallback for IP extraction in bot detector and IP ban middleware
- `geolocation` feature flag for optional GeoIP/MaxMind dependency in analytics and runtime

### Changed
- Simplify public `/health` endpoint to a lightweight DB-only check (fast for load balancers)
- Replace `tokio::process::Command("df")` disk usage with synchronous `libc::statvfs` syscall
- Make `CliService` conditionally compiled behind `cli` feature flag in logging crate
- Reduce default tokio features in workspace (remove `fs`, `process`, `signal` from default set)
- Replace blocking `std::sync::Mutex` with `tokio::sync::Mutex` in Gemini AI provider to prevent tokio worker thread stalls
- Agent sub-processes now start with a clean environment (`env_clear`) instead of inheriting all parent secrets
- Filter system traces and unknown status from trace list by default

### Security
- Fix OAuth redirect URI bypass: full URLs can no longer match relative URI registrations
- Fix WebAuthn user ID spoofing: completion handler now verifies authenticated user identity via auth token instead of trusting query parameter
- Remove wildcard CORS headers from WebAuthn completion endpoint
- Enforce 120-second expiry on WebAuthn registration and authentication challenges
- Add Shannon entropy validation for PKCE code challenges
- Block internal/private IP addresses in OAuth resource URI validation
- Use constant-time comparison (`subtle` crate) for sync token authentication
- Block symlinks and hardlinks in tarball extraction with canonical path validation
- Unify authorization code error messages to prevent enumeration attacks

### Refactored
- **CLI architecture remediation**: eliminate all `unwrap_or_default()`, `unsafe`, unlogged `.ok()`, and `println!()` violations across 8 CLI domains (admin, analytics, cloud, core, infrastructure, plugins, web, build)
- Split 14 oversized CLI files (>300 lines) into focused submodules — zero files now exceed the 300-line limit
- Extract magic numbers to named constants across analytics and infrastructure commands
- Refactor long functions (>75 lines) in analytics agents/show, sessions/live, and tools/show
- Replace `unsafe { std::env::set_var() }` in cloud profile/sync with safe `ProfileBootstrap::init_from_path()` config propagation
- Replace raw `std::env::var()` calls in cloud commands with Config-based alternatives
- **Struct consolidation**: rename duplicate `ToolModelConfig` (all-optional) to `ToolModelOverride`, resolve `Settings` collision into `ServicesSettings`/`DeploymentSettings`, deduplicate `RenderingHints` (CLI now imports from models crate)
- Convert `ToolContext` ID fields from raw `String` to typed identifiers (`SessionId`, `TraceId`, `AiToolCallId`)
- Convert image generation model ID fields from raw `String` to typed identifiers (`UserId`, `SessionId`, `TraceId`, `McpExecutionId`)
- **Eliminate inline SQL from CLI**: move 10 inline queries from `logs/show.rs`, `logs/export.rs`, and `logs/summary.rs` to `TraceQueryService` with dedicated query modules (`log_lookup_queries.rs`, `log_summary_queries.rs`)
- **Typed IDs for trace models**: replace 6 remaining `String` ID fields with typed identifiers (`LogId`, `AiRequestId`, `ExecutionStepId`) across `LogSearchItem`, `AiRequestListItem`, `AiRequestDetail`, `AuditLookupResult`, `ExecutionStep`, `AiRequestInfo`
- **DRY identifier definitions**: consolidate hand-written identifier structs into `define_id!()` macro invocations, removing ~2,500 lines of duplicated boilerplate across 14 identifier modules
- Consolidate shared utilities and per-crate `.sqlx/` caches for publish workflow
- Config cleanup: encapsulate visibility, remove dead code across config and logging crates
- **Code quality sweep across all layers** (139 files): remove clippy suppressions, fix forbidden constructs, eliminate silent error patterns
  - Remove `#[allow(clippy::*)]` suppressions by fixing underlying issues: `cognitive_complexity` (split functions), `too_many_arguments` (parameter structs), `struct_excessive_bools` (bitflags/enums), `print_stdout` (CliService::output/std::io::Write), `expect_used` (proper error propagation), `unnecessary_wraps`, `struct_field_names`, `empty_structs_with_brackets`, `option_option` (CategoryIdUpdate enum), `enum_variant_names`
  - Replace `CommandDescriptor` 6-bool struct with u8 bitflags pattern and const accessor methods
  - Introduce parameter structs: `TenantSessionParams`, `NonStreamingRequest`, `SessionStoreParams`, `ToolCallParams`, `TrackingParams`, `ReconcileSuccessParams`, `BuildContextParams`, `AuthCodeValidationParams`
  - Remove anyhow bridges in `AiError` and `AgentError`: replace `DatabaseError(#[from] anyhow::Error)` with `DatabaseError(String)`
  - Replace `println!`/`eprintln!` with `std::io::Write` across infra/logging CLI display and startup validation
  - Fix all `unwrap_or_default()` in CLI and domain code with explicit error handling
  - Fix silent error patterns: convert `let _ =` and `.ok()` to `tracing::warn!` or proper propagation across agent, mcp, scheduler, runtime, and API layers
  - Replace `Vec<EndpointRateLimit>` for rate limit config (eliminate struct_field_names)
  - Split `ProviderCapabilities` into `SchemaComposition` + `SchemaFeatures` sub-structs
  - Replace `process::exit()` with proper error propagation in CLI bootstrap
- Extract trace/logging queries into dedicated modules (`log_search_queries.rs`, `request_queries.rs`, `tool_queries.rs`, etc.)
- Remove dead `show_helpers.rs` and unused agent lib.rs clippy allow-list
- **Module visibility hardening**: convert `pub mod` to `pub(crate) mod` for internal modules across 7 domain crates (agent, ai, analytics, users, oauth, content, mcp) — reduces public API surface while preserving re-exports
- **Rename `models::ContentError` to `ContentValidationError`**: resolve naming collision with the operational `error::ContentError` in the content crate
- Fix `McpCspDomains` field references (`connect_domains` -> `connect`, `resource_domains` -> `resources`, etc.)
- Fix `BuildContextParams` call sites to use struct construction instead of positional args
- **Coding standards compliance sweep**:
  - Delete 20 dead `.rs` files and 4 dead `.sql` files not declared in any `mod.rs`
  - Convert 7 static `sqlx::query()` calls to compile-time verified `sqlx::query!()` / `sqlx::query_scalar!()` macros
  - Remove `unsafe` block in config manager: replace `std::env::set_var` with in-process `HashMap` for secret resolution
  - Remove `unsafe` block in health check: replace `libc::statvfs` FFI with `nix::sys::statvfs` safe wrapper
  - Split 6 files exceeding 400 lines into focused submodules: `audit_queries.rs`, `ai_trace_display.rs`, `secrets_bootstrap.rs`, `file_bundler.rs`, `deploy_steps.rs`, `profile_steps.rs`
- Fix `Arc<AnalyticsService>` to `Arc<dyn AnalyticsProvider>` coercion in session middleware
- Fix `CloudPaths` API consumers after `get_cloud_paths()` return type change
- Remove unused `_pool` parameter from `CleanupRepository::new_with_write_pool`

### Fixed
- Sub-process binary resolution now checks both `target/release` and `target/debug`, preferring the newest by mtime — matches justfile behavior so MCP servers and agents find the correct binary during development
- MCP binary validation uses dynamic bin directory resolution instead of hardcoding `target/release`
- Fix test compilation across `systemprompt-generator` and `systemprompt-sync`
- Remove needless `..Default::default()` in API JWT config
- Fix `bool as Option<bool>` invalid cast in trace list queries
- Populate AI trace summary fields (`total_cost`, `total_tokens`, `total_latency`) that were previously always zero

## [0.1.18] - 2026-03-05

### Changed
- Upgrade Rust edition from 2021 to 2024
- Reorder imports across all crates to comply with Rust 2024 edition formatting rules
- Change `unsafe_code` workspace lint from `forbid` to `deny`
- Parallelize prerender pipeline: concurrent source processing, item rendering, and content enrichment
- Replace regex-based TOC heading ID injection with string search (removes `regex` dependency from generator)

### Removed
- Remove TUI OAuth client seed data and configuration
- Remove TUI testing plan
