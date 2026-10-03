# Changelog

## [Unreleased]

### Added

- First release. The AI gateway services move here from `systemprompt_api::services::gateway` and the gateway repository bundle from `systemprompt_api::repository::GatewayRepositories`; the gateway-policy spec, YAML loader and ingestion, and the safety-scanner, route-selector and system-prompt-override contracts move here from `systemprompt_ai::services::gateway`. `PolicyResolver` lives in `policies` beside the spec it resolves.
- `GatewayAuditError` types the failures of the audit trail and settlement journal that were previously `anyhow::Error`.
