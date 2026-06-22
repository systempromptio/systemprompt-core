# Changelog

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
