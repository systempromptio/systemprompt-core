# Changelog

## [0.50.0] - 2026-09-10

### Fixed

- Replies go out on a guarded client. `serviceUrl` is read from the inbound activity payload, so it is chosen by the sender; the guarded resolver refuses a hostname that resolves into a blocked range, which the parse-time `validate_outbound_url` check cannot see. `TeamsError::ClientUnavailable` reports a guarded client that could not be built. Token acquisition stays on the injected client, because its URL is operator-configured and never caller-supplied.

## [0.48.0] - 2026-09-08

### Added

- `TeamsClient::with_endpoints`, `ActivityTokenVerifier::with_openid_url` and `TokenProvider::with_token_url` are unconditional constructors; Bot Framework endpoint constants come from `systemprompt_models`.

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

- Initial release. Microsoft Teams integration as a first-class inbound surface:
  Bot Framework activity-token validation (OpenID/JWKS, issuer and audience
  checks, `serviceUrl` binding), outbound OAuth2 client-credentials token
  acquisition, typed message/invoke activities normalized for dispatch, an
  SSRF-guarded outbound Bot Connector client, and Adaptive Card rendering.
  Registers the `teams` extension with a `teams_conversation_contexts` schema and
  the `teams` config prefix. The crate is fully opt-in (excluded from the facade
  `default` and `full` features).
