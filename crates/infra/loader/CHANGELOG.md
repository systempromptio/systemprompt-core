# Changelog

## [0.51.0] - 2026-09-11

### Added

- `vertex_discovery`: boot-time catalog discovery. `ServicesBootstrap::try_init_with_discovery(augment)` installs the registry through a `DiscoveryFuture` and `ServicesBootstrap::discovery_report()` exposes what it found. `discover(providers, secret, timeout)` decides per provider which `CatalogSource` may list it from the `ProviderCredential` its secret parses into, lists every publisher the embedded `VertexRateCard` names (paged `GET /v1beta1/publishers/{p}/models`, authenticated with the provider's service account), keeps serverless entries, and merges each one the card prices and the documentation still supports into the provider; an explicit catalog declaration always wins and one past the card's retirement line is kept and warned about. `ServicesConfig` is validated again after the merge so an unpriced route is never installed. `discover_with(providers, secret, timeout, Catalog { sources, card }, report)` is the injectable form; `default_sources(card)` builds the shipped source list. `CatalogSource`, `CatalogListing`, `DiscoveredModel`, `LaunchStage`, `DiscoveryError` and `SecretLookup` are public, and `merge::record_unseen` is generic over the set's hasher.
- The rate card is both price list and chat-capable allowlist: the publisher listing is Google's global catalog, not the project's entitlements, and no model is ever called to find any of this out. Unpriced, unpublished, retiring and failed publishers land in the `DiscoveryReport`.

## [0.50.0] - 2026-09-10

### Added

- `ServicesSourceBootstrap` resolves the services root from bundle sources at boot: fetch, verify, compose, swap, record. An empty `services.sources` serves the tree at `paths.services`, which is unchanged behaviour. `ServicesRootBootstrap` holds the resulting `ActiveServicesRoot`, and `ConfigLoader` reads the active root rather than the profile path directly.
- The `bundle` module packs, publishes, fetches and verifies a services bundle: `pack` and `extract` (both moved here, from the sync archive route and the CLI backup command respectively, and parameterised by the allowed directory list), `verify`, `cache`, `compose`, and `source` with HTTPS and OCI transports. OCI speaks the standard Bearer challenge and pulls a single layer of the bundle media type.
- Verification is ordered and every step is fatal: archive digest, manifest signature, per-file checksums, content hash. A bundle that fails any step is never installed and never cached.
- Composition is by id, not last-write-wins. Two bundles claiming the same marketplace, plugin, skill, rule, hook, artifact or base directory is a boot error naming both sources, because preferring one silently would make which access rules an instance enforces depend on the order of a YAML list.
- `ServicesProvenance` records why the active root is what it is — `Bundled`, `Fetched`, `LastGood` or `BundledFallback` — carrying the failure text on the two fallbacks, so an instance serving yesterday's bundle does not read as healthy.
- The cache is content-addressed under `<cache_dir>`, with `current` swapped by `rename` so a reader sees a whole composition or the previous one, never a half-copied tree. It keeps the last two versions per source and the last two composed roots, so a rollback needs no network.

## [0.48.0] - 2026-09-08

### Changed

- No functional change; the crate's `why` comments were re-cut by the comment-standards pass to state the hidden constraint and nothing else.

## [0.47.0] - 2026-09-06

### Changed

- providers with unresolved credential references are demoted to `surface: backend` before validation and omitted from model discovery. An uninitialized secret store remains an unknown state. Warnings identify the provider; `cloud doctor` reports missing secret names and checks the repository services tree outside a container.
## [0.44.0] - 2026-09-02

### Changed

- The loader reads the provider catalog and gateway routes from the services tree rather than the profile, and its gateway and catalog tests moved with them.

## [0.38.0] - 2026-08-25

### Changed

- A duplicate skill or plugin id across on-disk catalogue directories is a load error rather than a silent first-wins, and a root-config declaration shadowing an on-disk descriptor is logged.

## [0.30.0] - 2026-08-07

### Added

- A services-config include's `bridge_policy:` block merges into the target when the target declares none, so an instance policy declared once flows through composed configs.
- `profile.services.port_offset` is applied when the services config is loaded, shifting every locally-bound MCP and agent port and both port ranges; the existing port-range and conflict validators police the shifted values.

## [0.25.0] - 2026-07-27

### Added

- `ConfigLoader::reload` re-reads and re-validates the active profile's configuration, bypassing the `ConfigLoader::load` cache and refreshing it with the result. Commands that validate their own write need it, since a cached read after the write would report the pre-write state.

### Changed

- `ConfigLoader::load` memoises its result for the lifetime of the process, keyed by the resolved configuration path. Each call previously re-read the YAML, re-resolved the `includes:` graph, re-walked the skills, plugins, and marketplaces trees, and re-validated the result; boot alone called it a dozen times. A configuration edit now requires a restart. `ConfigLoader::load_from_path` and `ConfigLoader::validate_file` are unchanged and still read from disk on every call.

## [0.21.1] - 2026-07-17

### Changed
- Source files now carry a Business Source License 1.1 header referencing <https://systemprompt.io>.

## [0.19.0] - 2026-07-02

### Breaking

- The minimum supported Rust version is 1.94.

### Changed

- Workspace version bump; no API changes in this crate.

## [0.16.0] - 2026-06-22

### Breaking

- The minimum supported Rust version is 1.88.

## [0.14.0] - 2026-06-01

### Changed

- Workspace version bump; no API changes in this crate.

## [0.13.1] - 2026-06-01

### Changed

- Workspace version bump; no API changes in this crate.

## [0.13.0] - 2026-05-28

### Added

- Services-config loader auto-discovers `<services>/skills/<id>/config.yaml` and `<services>/plugins/<id>/config.yaml` and inserts them into `ServicesConfig.skills.skills` / `ServicesConfig.plugins.plugins` at load time. Marketplace / plugin `skills.include` and `mcp_servers.include` references resolve against the on-disk catalogue without each tenant duplicating every skill or plugin id under `services/config/config.yaml`.

### Changed

- `config_loader::mod` reads `PluginComponentRef` explicitly when materialising `SkillConfig` defaults rather than depending on the type-inferred sequence shape, matching the unified entity-id reference list shape now used across the services config.
- Marketplace validation resolves `mcp_servers.include` ids against the top-level `services.mcp_servers` catalogue at load time, matching the existing `skills` / `agents` / `plugins` shape on `MarketplaceConfig`.

## [0.12.0] - 2026-05-27

### Changed

- Workspace version bump; no API changes in this crate.

## [0.11.0] - 2026-05-20

### Changed
- Workspace-aligned release. Loader surface unchanged.

## [0.9.2] - 2026-05-14

### Added
- `expose-internals` Cargo feature gating test-only entry points such as `ConfigLoader::load_from_content` for use by dependent crates outside `cfg(test)`.

### Changed
- Split `config_loader` and `extension_loader` into submodules (`includes`, `merge`, `types`, `manifest`, `result`) for clearer separation between parsing, merging, and result types.

## [0.2.0] - 2026-04-15

### Breaking
- **Breaking:** `ConfigLoader` no longer auto-appends discovered agent files to the root `config.yaml` `includes:` list; migrate by listing every include explicitly in `config.yaml`.
- **Breaking:** Removed `EnhancedConfigLoader`, `IncludeResolver`, `ConfigLoader::discover_and_load_agents`, and `ConfigWriter::add_include`; migrate by using `ConfigLoader` directly with explicit `includes:` entries.

### Added
- Recursive `includes:` resolution with cycle detection.

### Changed
- Consolidated config loading into a single `ConfigLoader` with static-method shims preserving the prior public API.

## [0.1.18] - 2026-03-27

### Changed
- Upgraded to the Rust 2024 edition.

## [0.1.1] - 2026-02-03

### Added
- `ExtensionLoader::resolve_bin_directory()` utility that picks `target/debug` or `target/release` based on binary modification time.

### Fixed
- Resolved a clippy `unnested_or_patterns` warning in `resolve_bin_directory`.

## [0.1.0] - 2026-02-02

### Changed
- First stable release at the unified workspace version.

## [0.0.13] - 2026-01-27

### Changed
- Version bump for workspace consistency.

## [0.0.11] - 2026-01-26

### Removed
- Removed the standalone secrets loader; secrets are now loaded through the config system.

## [0.0.3] - 2026-01-22

### Added
- Migration system infrastructure.

### Fixed
- Schema validation for VIEW-based schemas.

## [0.0.2] - 2026-01-22

### Changed
- Adopted a distributed schema registration pattern in which each domain crate owns its SQL schemas via the `Extension` trait.

### Removed
- Centralised module loaders previously hosted in this crate.

### Fixed
- Corrected `include_str!` paths that pointed outside the crate directory so the crate compiles standalone when downloaded from crates.io.

## [0.0.1] - 2026-01-21

### Added
- Initial release.
