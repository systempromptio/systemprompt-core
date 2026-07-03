# Changelog

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
