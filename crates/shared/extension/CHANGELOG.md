# Changelog

## [0.47.0] - 2026-09-06

### Added

- A migration slot can be declared spent. `NNN_<name>.tombstone` (or `NNN-MMM_<name>.tombstone` for a retired chain) records the number with no SQL and a prose body; `build.rs` treats the slot as occupied, so refilling a spent number fails the build rather than a deployment. A shipped migration keeps a tracking row in every established database forever, and deleting the file did not give the number back — refilling it looked exactly like editing the migration that used to live there, and the runner reported "has been edited since it was applied" naming a file nobody could find, with both offered remedies wrong.
- `MigrationError::MigrationSlotReused` names the file that held the slot and the one that wants it. The recorded `name` of an applied version is finally compared against the file now occupying it; the mismatch is tolerated by `--allow-checksum-drift` the same way a checksum mismatch already is.

### Changed

- The runner never executes, records, or checksums a tombstoned slot, and fresh-install stamping skips it — a database that never ran the migration should not carry a row claiming it did.
- An applied version no file claims any more is reported as orphaned and warned about, never fatal. Every database predating this carries rows for migrations since deleted, so refusing to boot on those would strand every established install; adding the tombstone is what clears the warning.

### Fixed

- `PendingMigration.no_tx` reports the migration's real transaction mode. It was hardcoded `false` at both construction sites, so every no-transaction migration was reported as transactional.

## [0.27.0] - 2026-07-29

### Breaking

- **Breaking:** `GatewayRequestGuard::check` and `run_gateway_guards` take a `GatewayGuardRequest` (user id, requested model, resolved route id, provider, streaming flag) instead of a bare user id, so a guard can enforce per-plan model and route entitlement, not only balance checks. Migrate by reading `request.user_id` and matching on the new fields as needed.
- **Breaking:** `GatewayDenyReason` gains `kind: GatewayDenyKind` (`Quota` | `Forbidden`). `GatewayDenyReason::new` keeps `Quota`, so existing denials still map to 429 + `retry-after`; the new `GatewayDenyReason::forbidden` maps to a 403 with no `retry-after`, for entitlement denials where retrying can never help.
- **Breaking:** `ExtensionRegistry::api_extensions` is replaced by `api_routers`, which returns `Vec<(Arc<dyn Extension>, ExtensionRouter)>` — each extension paired with its already-built router. Migrate by iterating the pairs instead of calling `router()` again on each extension.
- **Breaking:** `Extension::has_router` is removed. It answered a boolean by building the whole router and discarding it, so any implementation that allocated, spawned a task, or logged did so an extra time per startup. Migrate by using `api_routers`, which filters on router presence as it builds.
- **Breaking:** `ExtensionRegistry::enabled_api_extensions` is removed; it had no callers and shared the same defect.

### Fixed

- Extension routers are built once per startup instead of twice. Background tasks spawned during router construction are no longer duplicated for the lifetime of the process.

## [0.24.0] - 2026-07-26

### Breaking

- **Breaking:** `SchemaDefinition.table` is `Option<String>`, so a definition may declare DDL that creates no table. Migrate by matching on the option at any site that reads `.table`; `SchemaDefinition::new(table, sql)` is unchanged and yields `Some`.

### Added

- `SchemaDefinition::sql_only(sql)` declares DDL with no owning table, such as shared trigger functions.

### Removed

- **Breaking:** the unused typestate authoring API is removed: `ExtensionBuilder`, `TypedExtensionRegistry`, the `SchemaExtensionTyped` / `ApiExtensionTyped` / `JobExtensionTyped` / `ProviderExtensionTyped` / `ConfigExtensionTyped` traits and `SchemaDefinitionTyped`, the `ExtensionType` / `ExtensionMeta` / `Dependencies` / `DependencyList` / `NoDependencies` traits, the `hlist` (`TypeList` / `Subset`) machinery, the `AnyExtension` wrappers, and `HasExtension`. Extensions are authored through the `Extension` trait and registered with `register_extension!`, which is unchanged.

### Added

- `GatewayRequestGuard`, `GatewayDenyReason`, `run_gateway_guards`, and the `register_gateway_guard!` macro: an inventory-collected policy hook consulted on every gateway request after the quota precheck, so an extension can enforce its own admission policy (a per-user credit balance, for instance) without core knowing about it.

## [0.22.0] - 2026-07-20

### Added

- `FrameOptions` enum modelling the `X-Frame-Options` response header (`DENY`, `SAMEORIGIN`).

## [0.21.1] - 2026-07-17

### Changed
- Source files now carry a Business Source License 1.1 header referencing <https://systemprompt.io>.

## [0.20.0] - 2026-07-15

### Added

- Extensions can override the framing policy for their routes: `ExtensionRouter::with_frame_options(FrameOptions)` (or `stamp_frame_options` on a route subtree) records a typed `FrameOptionsOverride` response extension honoured by the global security-headers middleware — `SAMEORIGIN`/`DENY`, or header removal for `FrameOptions::AllowAll`, always paired with the matching `frame-ancestors` CSP directive. The profile default still applies everywhere else.

## [0.19.0] - 2026-07-02

### Breaking

- The minimum supported Rust version is 1.94.
- The unused `Contains` and `NotSame` hlist traits are removed from the public API, and `LoaderError::CircularDependency` is removed in favour of `DependencyCycle` (dependency-cycle detection is unified into `topo_sort`).

## [0.16.0] - 2026-06-22

### Breaking

- **Breaking:** `TypedExtensionRegistry::has_type` and `TypedExtensionRegistry::get_typed` are removed; they consulted a type index that was never populated and always reported absent. Dependency ordering is enforced at compile time by `ExtensionBuilder`.
- **Breaking:** `DependencyList::validate` and the `MissingDependency` struct (under `types`) are removed for the same reason; `DependencyList::dependency_ids` is unchanged.
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
- Workspace-aligned release. Extension trait surface unchanged; downstream crates continue to register via `register_extension!` and `extension_migrations!()`.

## [0.9.2] - 2026-05-14

### Breaking
- Removed `Extension::migration_weight()` and `SchemaExtensionTyped::migration_weight()`; extension ordering is the dependency graph only.
- Removed `Extension::owned_tables()`; an extension's owned tables are derived from the `CREATE TABLE` statements in its `schemas()`.
- Removed `LoaderError::InvalidDependencyOrdering`.

### Added
- `LoaderError::DuplicateTableOwner`, `CrossExtensionTableNotOwned`, and `SeedInsertNotIdempotent`.
- `SchemaDefinition::with_schema` and `schema_name()` for non-`public` schema-qualified tables.

### Changed
- Align crate version with the `systemprompt-core` workspace release cadence.

## [0.1.21] - 2026-04-02

### Changed
- Move `RESERVED_PATHS` to the `registry` module and re-export from `typed_registry`.

## [0.1.18] - 2026-03-27

### Changed
- Upgrade to Rust 2024 edition.
- Split `lib.rs` and `registry.rs` into focused submodules.

## [0.1.12] - 2026-02-10

### Added
- `AssetType::Html` variant for declaring HTML assets.
- `AssetDefinition::html()` convenience constructor.

## [0.1.10] - 2026-02-06

### Added
- `SiteAuthConfig` type for declaring site-wide authentication requirements.
- `Extension::site_auth()` method, defaulting to `None`.
- `Extension::has_site_auth()` predicate method.
- `SiteAuthConfig` re-exported from the prelude.

## [0.1.0] - 2026-02-02

### Changed
- First stable release.

## [0.0.13] - 2026-01-27

### Changed
- Version bump for workspace consistency.

## [0.0.3] - 2026-01-22

### Added
- `Migration` struct for versioned extension migrations.
- `Extension::is_required()` method to mark core extensions as non-disableable.
- `Extension::migrations()` method for declaring versioned migrations.
- `Extension::has_migrations()` helper.
- `LoaderError::MigrationFailed` variant.
- `enabled_extensions()`, `enabled_schema_extensions()`, `enabled_api_extensions()`, and `enabled_job_extensions()` filters on the registry.

### Fixed
- Schema validation for VIEW-based schemas.

## [0.0.2] - 2026-01-22

### Changed
- Each domain crate now owns its SQL schemas via the `Extension` trait, replacing the centralized loader in `systemprompt-loader`.

### Fixed
- `include_str!` paths that pointed outside the crate directory.
- Standalone compilation when consumed from crates.io.

## [0.0.1] - 2026-01-21

### Added
- Initial release.
