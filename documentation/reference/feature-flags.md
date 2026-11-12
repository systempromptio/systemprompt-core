# Facade feature flags

The `systemprompt` facade exposes additive Cargo features. The default is `core`.
Disable defaults with `default-features = false` when selecting an independent feature.

```toml
[dependencies]
systemprompt = { version = "0.81", features = ["api"] }
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
| `files-pdf` | `dep:systemprompt-files`, `systemprompt-files/pdf`, `systemprompt-api?/pdf`, `systemprompt-cli?/pdf` |
| `files-collab` | `dep:systemprompt-files`, `systemprompt-files/collab`, `systemprompt-runtime?/collab`, `systemprompt-api?/collab` |
| `files-collab-loro` | `files-collab`, `systemprompt-runtime?/engine-loro` |
| `agent-runs` | `dep:systemprompt-agent-runs`, `systemprompt-api?/agent-runs`, `systemprompt-cli?/agent-runs` |
| `otel` | `systemprompt-api?/otel`, `systemprompt-cli?/otel`, `systemprompt-scheduler?/otel` |
| `bridge` | `systemprompt-api?/bridge`, `systemprompt-cli?/bridge` |
| `slack` | `dep:systemprompt-slack`, `systemprompt-api?/slack`, `systemprompt-cli?/slack` |
| `teams` | `dep:systemprompt-teams`, `systemprompt-api?/teams`, `systemprompt-cli?/teams` |
| `notifications` | `dep:systemprompt-notifications`, `systemprompt-runtime?/notifications`, `systemprompt-api?/notifications` |
| `brain` | `dep:systemprompt-brain`, `notifications`, `systemprompt-runtime?/brain`, `systemprompt-api?/brain` |
| `full` | `api`, `mcp`, `cloud`, `cli`, `dep:systemprompt-agent`, `dep:systemprompt-ai`, `dep:systemprompt-mcp`, `dep:systemprompt-oauth`, `dep:systemprompt-users`, `dep:systemprompt-content`, `analytics`, `dep:systemprompt-marketplace`, `dep:systemprompt-scheduler`, `dep:systemprompt-generator`, `logging`, `systemprompt-logging/cli`, `config`, `dep:systemprompt-files`, `files-office`, `files-pdf`, `files-collab-loro`, `agent-runs`, `otel`, `bridge`, `notifications`, `brain`, `loader`, `events`, `storage`, `client`, `security` |

`dep:` selects an optional dependency; an unprefixed name selects another feature.
`dependency/feature` enables a feature on that dependency. `full` excludes the opt-in
`slack` and `teams` integrations. Enable them explicitly when required.

`files-office` adds the Word, Excel, PowerPoint and OpenDocument codecs to the document store; the HTML, Markdown and text codecs and Markdown export are always present. `files-pdf` adds PDF export and its embedded fonts; without it a PDF export is refused with `unsupported_op`. `files-collab` mounts the live co-editing routes and builds the `CollabService`; without an engine every session route answers `503 collab_engine_unavailable`. `files-collab-loro` adds the Loro engine (`systemprompt-files/collab-loro`), the only engine core ships. The document-scoped live channel and awareness route are mounted with the store itself and need no collab feature.

`slack` and `teams` mount the `/slack` and `/teams` inbound routes. `agent-runs` mounts `/api/v1/agent-runs` and wires the run launcher into hosted agents. `otel` mounts the gateway's OTLP/HTTP ingest routes and registers the `otlp_export` job. `bridge` mounts the device-credential consumer routes the bridge reports installation evidence through. Without a feature its routes are not mounted and its job is not registered.

`notifications` adds notification streams, rules, the per-user feed and their outbox consumer. `brain` adds the brain: its capture consumer, gateway recall, routes and jobs (see [the brain](../brain.md)); it implies `notifications`, and a profile with `brain.enabled: false` composes none of it.

The `mcp` feature exposes `rmcp`; the `systemprompt-mcp` domain crate is included by
`full`. `runtime` enables the CLI-backed runtime builder. The `api` dependency on
`systemprompt-runtime` enables its `geolocation` feature.

## Panic strategy

A build with `panic = "abort"` ends the process on any panic. A build with unwinding contains a panic to the job, background task or request that raised it; a request that panics receives the standard `500` error envelope and the panic message is logged.

## Examples and API documentation

The bundled `extension`, `database`, `api` and `cli` examples require `core`, `database`,
`api` and `cli`, respectively. Published docs.rs documentation enables all features,
including integrations excluded from `full`.

See [extensions](../concepts/extensions.md) for registration contracts and
[HTTP API](http-api.md) for server routes.
