# Changelog

All notable changes to `systemprompt-marketplace` will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
