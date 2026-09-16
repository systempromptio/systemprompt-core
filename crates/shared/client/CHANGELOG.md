# Changelog

## [0.53.0] - 2026-09-15

### Breaking

- **Breaking:** `SystempromptClient::list_artifacts` / `list_all_artifacts` return `Vec<systemprompt_models::a2a::Artifact>`; `send_message` takes `&AgentName` and `&a2a::Message`.
- **Breaking:** `RemoteCliRequest { token: &SessionToken, context: Option<&ContextId> }` replaces the string fields; `EventStreamSetup` is removed. A stream that ends before an `ExitCode` event, or delivers an undecodable event, is `ClientError::ServerUnavailable`; a non-2xx response is `ClientError::ApiError`. The stream is driven by `sse-stream`; `reqwest-eventsource` is no longer a dependency.

### Added

- `guarded` module: the SSRF-guarded outbound `reqwest` client builder (previously `systemprompt_models::net::client`).

### Fixed

- `ClientError::from_response` no longer stores the response body twice.

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
- Workspace-aligned release. No public API change beyond inherited typed-identifier and rustdoc standards work.

## [0.10.1] - 2026-05-15

### Changed
- Version bump for workspace consistency.

## [0.9.2] - 2026-05-14

### Changed
- Version bump for workspace consistency.

## [0.1.23] - 2026-04-14

### Fixed
- `SystempromptClient::send_message` uses the A2A v1.0.0 method name `SendMessage` instead of the legacy `message/send`.

## [0.1.18] - 2026-03-27

### Changed
- Upgrade to Rust 2024 edition.

## [0.1.0] - 2026-02-02

### Changed
- First stable release at workspace-aligned `0.1.0`.

## [0.0.13] - 2026-01-27

### Changed
- Version bump for workspace consistency.

## [0.0.3] - 2026-01-22

### Added
- Migration system infrastructure.

### Fixed
- Schema validation accepts `VIEW`-based schemas.

## [0.0.2] - 2026-01-22

### Changed
- Each domain crate owns its SQL schemas via the `Extension` trait; centralized module loaders removed from `systemprompt-loader`.

### Fixed
- `include_str!` paths no longer point outside the crate directory, so the crate compiles standalone when downloaded from crates.io.

## [0.0.1] - 2026-01-21

### Added
- Initial release.
