# Changelog

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
