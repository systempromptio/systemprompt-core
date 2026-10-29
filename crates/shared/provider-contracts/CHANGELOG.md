# Changelog

## [0.76.0] - 2026-10-29

### Breaking

- **Breaking:** `CodecError` gains `TooLarge { what, size, limit }`, mapped to `DocumentsError::TooLarge` and the `too_large` refusal. Migrate exhaustive matches by adding the variant.

## [0.75.0] - 2026-10-28

### Breaking

- `EditOp` gains `SetParagraph`, `InsertBlock`, `UpdateTable`, `SetSection`, `SetStyle` and `ApplyTheme`. Migrate by adding arms (or a wildcard) to exhaustive matches.
- `EditOp` gains `SetMarks`, `SetBlockType`, `InsertTable`, `TableInsertRow`, `TableInsertCol`, `TableDelete` and `InsertImage`, with their payload types (`SetMarks`, `MarkSpec`, `SetBlockType`, `TextBlockType`, `InsertTable`, `TableInsert`, `TableDelete`, `InsertImage`) and `EditOp::{is_rich, rich_target, rich_guard}`; `resolve_targets` resolves their targets. Exhaustive matches need new arms.
- `DocumentFormat` gains `File` (any upload with no editable format), `CodecError` gains `Refused` and `Payload`, and Markdown `materialise` returns `Result`. Migrate by adding arms (or a wildcard) to exhaustive matches and handling the error.

## [0.74.0] - 2026-10-27

### Added

- `documents::DocumentCodec` and `documents::DocumentTemplateProvider` seams, registered with `register_document_codec!` and `register_document_template_provider!` and discovered highest priority first; the format, projection and edit-op types (`EditOp`, `NodeTarget`, `ContentAs`, `NodeKind`), `resolve_targets`, and `DocumentCodec::insert_kinds`.

## [0.73.0] - 2026-10-25

### Breaking

- `ConfigPlane::apply` has a default built on `resolve_apply`, `read_current` and `write`.
- `ConfigPlane` gains `sources`, `read`, `write` and `render_declarations` (defaulted), so an exhaustive implementor list or a wrapper that forwards every method must add them.

### Added

- `ChangeSource`, the closed provenance vocabulary `code:<file>` | `bundle:<name>` | `console:<user>` | `migration`, with `Display`/`FromStr`/serde in that text form.
- `DeclaredEntity`/`DeclaredEntities` (value, per-entity hash, origin), `ResolvedEntity`/`ResolvedEntities` and `entity_hash`.

## [0.71.0] - 2026-10-21

### Added

- `ConfigPlaneSet`, `EffectiveAuthority` and `PlaneAuthority`; `ConfigPlane::depends_on`, `effect` and `export`, scoped apply (`EntityScope`), `ApplyRequest`, `PlaneTx` and the writer token.
- `DbBudget` (returned by `Job::db_budget()`) and `run_batched` with `BatchPlan` and `BatchedRun`, the bounded batch loop.

### Changed

- **Breaking:** `ConfigPlane::read_current`, `apply` and `export` take the host transaction, `apply` takes an `ApplyRequest`, and `ApplyMode`, `ChangeActor` and `ConfigPlaneError` gain `SeedIfEmpty`, `Seed` and `Unsupported`/`UnknownDependency`/`DependencyCycle`/`PlanesUnavailable`.

## [0.70.0] - 2026-10-18

### Added

- `ConfigPlane` contract, declaration diff and `ConfigPlaneError`.
- `Job::default_schedule()`.

## [0.63.0] - 2026-10-07

### Breaking

- `JobContext`, `PageContext`, `PagePrepareContext`, `ContentDataContext`, `FrontmatterContext` and `ExtenderContext` carry the host's handles in a typed `Dependencies` map instead of `dyn Any` slots: `JobContext::new(actor, Dependencies)`, `PageContext::new(page_type, web_config, &Dependencies)`, `PagePrepareContext::new(web_config, &Dependencies, dist_dir)`, and the other constructors take `&Dependencies` where they took the pool. `db_pool::<T>()`, `app_context::<T>()`, `app_paths::<T>()`, `content_config::<T>()` and the `*_arc()` accessors are removed; `ctx.get::<T>()` returns `Result<&T, MissingDependency>`, which converts into the new `ProviderError::MissingDependency`. Migrate by replacing `ctx.db_pool::<DbPool>().ok_or_else(..)?` with `ctx.get::<DbPool>()?` and building contexts with `Dependencies::new().with(value)`.
- `ProviderError` keeps causes as typed sources: `ConfigurationLoad { context, source }`, `InvalidParameter { key, value, source }` and `Rendering { context, source }` are new and `Internal` wraps a boxed error instead of a `String`.
- `ToolProviderError::ConnectionFailed` is removed (never produced); `ToolProviderError::Execution` is new and `Internal` wraps a boxed error. Migrate by dropping the arm and matching `Execution`.
- `ToolContext::new(actor)` no longer takes a token; `auth_token` is `Option<JwtToken>`, set with `.with_auth_token(token)`. `ToolCallRequest.tool_call_id` is `AiToolCallId` and `ToolProvider::find_tool` takes `&McpToolName`.
- `SiteI18nConfig::validate` returns `WebConfigError::UnsupportedDefaultLocale { default_locale }` instead of a `String`.

### Added

- `FrameOptions` is defined here (moved from `systemprompt-extension`) so `systemprompt-models` no longer depends on the extension crate.
- `DynPagePrerenderer` (`Arc<dyn PagePrerenderer>`) is exported from this crate.

## [0.60.0] - 2026-09-23

### Added

- `JobResult::is_idle`: a successful run with `with_stats(0, 0)` and no message, which the scheduler does not record.
- `Job::configured` (default `true`): a job that answers `false` is not cron-scheduled.

## [0.53.0] - 2026-09-15

### Breaking

- **Breaking:** `ToolProvider::list_tools` returns `ToolInventory { tools, failed_servers }` — a typed partial outcome — instead of a bare `Vec<ToolDefinition>` that silently omitted servers that failed to list. Migrate by reading `.tools` and treating a non-empty `failed_servers` as a failure where the full inventory matters; `ToolInventory::complete(tools)` builds a full one.

- **Breaking:** `ToolDefinition::model_config` is `Option<ToolModelConfig>` (the type moved here from `systemprompt_models::ai`, which re-exports it); `ToolDefinition::new` takes a `McpServerId`; `ToolDefinition::service_id` is a `McpServerId`.
- **Breaking:** `ToolProvider::refresh_connections` takes `&AgentName`.
- **Breaking:** `ToolProviderError::Config` is removed; `web_config::WebConfigError::Io { path }` carries a `PathBuf`.

### Removed

- The `llm` module (`LlmProvider`, `ToolExecutor`, `ChatRequest`, `ChatResponse`, `ChatMessage`, `ChatRole`, `SamplingParameters`, `TokenUsage`, `ToolExecutionContext`, `ChatStream`, `LlmProviderError`) — no implementor or consumer existed. Inference goes through `systemprompt_models::ai::AiProvider`.

## [0.44.0] - 2026-09-02

### Added

- The `FileStorage` contract, so domain crates write files through a trait the app layer wires to a backend rather than touching a path themselves.

## [0.27.0] - 2026-07-29

### Added

- `JobContext::get_parameter_parsed::<T>` parses a job parameter, returning `Ok(None)` when absent and `ProviderError::Configuration` when present but unparseable, so a mistyped override fails the run instead of falling back.

## [0.25.0] - 2026-07-27

### Added

- `Job::schedulable` lets a job declare that it exists only as an inline step of a larger pipeline job and so can never legitimately carry its own `scheduler.jobs` entry. The scheduler skips its unscheduled-job warning for such a job. Defaults to `true`, so no existing implementation needs to change.

## [0.23.0] - 2026-07-24

### Added

- `JobContext::enforce()` and `JobContext::with_enforce()` carry the job configuration's enforcement consent. A job whose actions are destructive or outward-facing must take them only when `enforce()` is `true`; the default is `false`.

## [0.21.1] - 2026-07-17

### Changed
- Source files now carry a Business Source License 1.1 header referencing <https://systemprompt.io>.

## [0.19.0] - 2026-07-02

### Breaking

- The minimum supported Rust version is 1.94.
- `ToolProvider::call_tool` takes a typed `McpServerId`; `ToolProviderError` configuration variants carry their source instead of flattening to a string; target/routable lookups adopt the `list_` prefix.

## [0.16.0] - 2026-06-22

### Breaking

- Error enum tuple variants that wrapped a bare message string are now struct variants with a named `message` field; match arms and constructors change from `Error::Foo(msg)` to `Error::Foo { message: msg }`.
- The minimum supported Rust version is 1.88.

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
- Workspace-aligned release. Provider trait definitions track the tenancy strip in `domain/ai` and `domain/oauth`: provider call sites no longer thread a runtime `tenant_id`.

## [0.9.2] - 2026-05-14

### Changed
- Normalized changelog format to maintainer style with explicit categories.

## [0.1.0] - 2026-02-02

### Changed
- Aligned to workspace 0.1.0 release.

## [0.0.13] - 2026-01-27

### Changed
- Workspace version bump.

## [0.0.3] - 2026-01-22

### Added
- Migration system infrastructure.

### Fixed
- Schema validation now handles VIEW-based schemas.

## [0.0.2] - 2026-01-22

### Changed
- Each domain crate now owns its SQL schemas via the `Extension` trait.

### Removed
- Centralized module loaders from `systemprompt-loader`.

### Fixed
- Corrected `include_str!` paths that pointed outside the crate directory.
- Ensured the crate compiles standalone when downloaded from crates.io.

## [0.0.1] - 2026-01-21

### Added
- Initial release.
