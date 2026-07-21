# Changelog

## [0.22.0] - 2026-07-21

### Breaking

- `ArtifactEntry.plugin_id` and `DiskArtifactConfig.plugin_id` are removed; a plugin selects its artifacts through `PluginConfig.artifacts`, so one artifact can belong to several plugins. Migrate by deleting `plugin_id:` from each `services/artifacts/<id>/config.yaml` and listing the artifact id under the owning plugin's `artifacts.include`.

### Added

- `selects_artifact` and `artifact_owners` expose plugin-to-artifact selection as the single distribution gate shared by manifest assembly and bundle building.
- Plugin bundles carry their selected artifacts as `artifacts/<id>.json` entries.

### Fixed

- Artifacts whose every owning plugin is filtered out for a user are no longer shipped in that user's manifest; orphan pruning runs after per-user filtering rather than before it.

## [0.21.1] - 2026-07-17

### Changed
- Source files now carry a Business Source License 1.1 header referencing <https://systemprompt.io>.

## [0.19.0] - 2026-07-02

### Breaking

- `MarketplaceCandidate::new` takes an additional `artifacts: Vec<ArtifactEntry>` argument, and `CatalogContent::into_parts` returns a 4-tuple including the artifact set. Migrate by passing `Vec::new()` / destructuring the extra element where artifacts are not used.

### Added

- `load_artifacts` loads `services/artifacts/<id>/config.yaml` entries into signed `ArtifactEntry` records; an artifact with empty HTML content or no `mcp_tools` is dropped with a warning.
- Manifest assembly scopes artifacts by the marketplace `artifacts` include list and drops artifacts whose owning plugin did not survive plugin selection.

## [0.18.0] - 2026-07-01

### Changed

- The bridge manifest emits the gateway MCP URL (`{base}/api/v1/mcp/{name}/mcp`) for an external server that declares an `external_auth` accessor, instead of the provider's raw endpoint, so the provider URL and per-user token stay server-side. An external server without an accessor keeps its raw endpoint.

## [0.17.0] - 2026-06-24

### Fixed

- A plugin that references a managed MCP server which is defined but `enabled: false` no longer logs a spurious "unknown server" warning during bundle assembly. The disabled server is quietly omitted from the plugin's `.mcp.json` (at `debug`) and reappears when re-enabled; a reference to a server not defined at all still warns.

## [0.16.0] - 2026-06-22

### Breaking

- The minimum supported Rust version is 1.88.

## [0.14.0] - 2026-06-01

### Changed

- Workspace version bump; no API changes in this crate.

All notable changes to `systemprompt-marketplace` will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.13.1] - 2026-06-01

### Changed

- The crate owns the plugin-bundle contract: `bundle::build_plugin_bundle` assembles a plugin from a `PluginConfig` and the resolved catalogue, and both the manifest (`load_plugins`) and the plugin-file byte route build from that one source so their hashes and served bytes cannot drift. A spec whose references resolve to no content is skipped rather than emitting an empty, malformed entry.
- `MarketplaceService` resolves the active marketplace solely from `settings.default_marketplace_id` (or the single configured marketplace); the implicit `"default"`-id fallback is removed and `resolve_default` / `active` share one selector.

## [0.13.0] - 2026-05-28

### Changed

- Workspace version bump; no API changes in this crate.

## [0.12.0] - 2026-05-27

### Changed

- Workspace version bump; no API changes in this crate.

## [0.11.0] - 2026-05-20

### Changed
- Workspace version alignment; no consumer-facing API change beyond inherited typed-identifier and rustdoc standards work.

## [0.9.2] - 2026-05-14

### Changed
- Documented the inventory-based filter registration flow on `MarketplaceFilterRegistration` and the `register_marketplace_filter!` macro.

## [0.9.1] - 2026-05-12

### Added
- `HookEntry` field on `MarketplaceCandidate` so hooks participate in per-user filtering alongside plugins, skills, agents, and managed MCP servers.

## [0.9.0] - 2026-05-11

### Added
- Initial release of `systemprompt-marketplace`.
- `MarketplaceFilter` async trait that the bridge manifest handler invokes before signing the canonical view.
- `MarketplaceCandidate` bundle of plugins, skills, agents, and managed MCP servers passed to each filter.
- `AllowAllFilter` passthrough default for deployments without an ACL backend.
- `MarketplaceFilterError` with `Backend`, `UnknownUser`, and `Policy` variants.
- `MarketplaceFilterRegistration` inventory slot and `register_marketplace_filter!` macro for compile-time extension wiring with priority ordering.
