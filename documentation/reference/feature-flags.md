# Facade feature flags

The `systemprompt` facade exposes additive Cargo features. The default is `core`.
Disable defaults with `default-features = false` when selecting an independent feature.

```toml
[dependencies]
systemprompt = { version = "0.76", features = ["api"] }
```

## Feature matrix

Direct dependencies and feature implications from [the facade manifest](../../systemprompt/Cargo.toml).
Cargo enables the transitive dependencies of each selected feature.

| Feature | Direct dependencies and feature implications |
|---------|---------------------------------------------|
| `default` | `core` |
| `core` | `dep:systemprompt-traits`, `dep:systemprompt-models`, `dep:systemprompt-identifiers`, `dep:systemprompt-extension`, `dep:systemprompt-template-provider` |
| `database` | `dep:systemprompt-database`, `dep:sqlx` |
| `config` | `dep:systemprompt-config` |
| `mcp` | `dep:rmcp` |
| `api` | `core`, `database`, `dep:systemprompt-api`, `dep:systemprompt-gateway`, `dep:systemprompt-runtime`, `dep:systemprompt-oauth-issuance`, `dep:axum` |
| `cloud` | `dep:systemprompt-cloud` |
| `logging` | `dep:systemprompt-logging` |
| `loader` | `dep:systemprompt-loader` |
| `events` | `dep:systemprompt-events` |
| `storage` | `dep:systemprompt-storage` |
| `client` | `dep:systemprompt-client` |
| `security` | `dep:systemprompt-security`, `dep:systemprompt-governance` |
| `cli` | `dep:systemprompt-cli` |
| `runtime` | `cli`, `dep:systemprompt-extension` |
| `analytics` | `dep:systemprompt-analytics` |
| `files-office` | `dep:systemprompt-files`, `systemprompt-files/office` |
| `files-collab` | `dep:systemprompt-files`, `systemprompt-files/collab`, `systemprompt-runtime?/collab`, `systemprompt-api?/collab` |
| `files-collab-loro` | `files-collab`, `systemprompt-runtime?/engine-loro` |
| `agent-runs` | `dep:systemprompt-agent-runs` |
| `slack` | `dep:systemprompt-slack` |
| `teams` | `dep:systemprompt-teams` |
| `full` | `api`, `mcp`, `cloud`, `cli`, `dep:systemprompt-agent`, `dep:systemprompt-ai`, `dep:systemprompt-mcp`, `dep:systemprompt-oauth`, `dep:systemprompt-users`, `dep:systemprompt-content`, `analytics`, `dep:systemprompt-marketplace`, `dep:systemprompt-scheduler`, `dep:systemprompt-generator`, `logging`, `systemprompt-logging/cli`, `config`, `dep:systemprompt-files`, `files-office`, `files-collab-loro`, `agent-runs`, `loader`, `events`, `storage`, `client`, `security` |

`dep:` selects an optional dependency; an unprefixed name selects another feature.
`dependency/feature` enables a feature on that dependency. `full` excludes the opt-in
`slack` and `teams` integrations. Enable them explicitly when required.

`files-office` adds the Word, Excel, PowerPoint and OpenDocument codecs to the document store; the HTML, Markdown and text codecs and PDF export are always present. `files-collab` mounts the live co-editing routes and builds the `CollabService`; without an engine every session route answers `503 collab_engine_unavailable`. `files-collab-loro` adds the Loro engine (`systemprompt-engine-loro`), the only engine core ships. The document-scoped live channel and awareness route are mounted with the store itself and need no collab feature.

The `mcp` feature exposes `rmcp`; the `systemprompt-mcp` domain crate is included by
`full`. `runtime` enables the CLI-backed runtime builder. The `api` dependency on
`systemprompt-runtime` enables its `geolocation` feature.

## Examples and API documentation

The bundled `extension`, `database`, `api` and `cli` examples require `core`, `database`,
`api` and `cli`, respectively. Published docs.rs documentation enables all features,
including integrations excluded from `full`.

See [extensions](../concepts/extensions.md) for registration contracts and
[HTTP API](http-api.md) for server routes.
