# Changelog

## [0.63.0] - 2026-10-07

### Breaking

- `signature::sign` returns `SlackResult<String>` and fails with `SlackError::SigningKey` when the signing secret is rejected.
- `SlackError::Signature` is replaced by `EmptySigningSecret` and `MissingSignaturePrefix`; `UnknownWorkspace`, `MalformedRequest` and `NoAgentRouted` are removed. Slack apps are resolved by `SlackWorkspaceId`.
- Manifest types are imported from `systemprompt-manifest`.

### Changed

- An empty signing secret is refused; `users.info` is decoded into typed structs.

### Fixed

- `user_info` falls back to `display_name` when `real_name` is null.

## [0.53.0] - 2026-09-15

### Changed

- `validate_config` returns `systemprompt_extension::ExtensionConfigError` (the renamed `ConfigError`). No behavioural change.

## [0.48.0] - 2026-09-08

### Added

- `SlackClient::with_base_url` and `with_users_info_url` are unconditional constructors.

### Removed

- The `test` Cargo feature.

## [0.21.1] - 2026-07-17

### Changed
- Source files now carry a Business Source License 1.1 header referencing <https://systemprompt.io>.

## [0.19.0] - 2026-07-02

### Breaking

- The minimum supported Rust version is 1.94.

### Changed

- Workspace version bump; no API changes in this crate.

## [0.17.0] - 2026-06-24

### Added

- Initial release. Slack integration as a first-class inbound surface: signature
  verification, typed Events API / slash-command / interactivity payloads,
  declarative `services/slack/*.yaml` app configuration, an SSRF-guarded outbound
  Web API client, and Block Kit rendering. Registers the `slack` extension with a
  `slack_channel_contexts` schema and the `slack` config prefix.
