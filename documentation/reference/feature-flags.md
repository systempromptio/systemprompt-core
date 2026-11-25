# Facade feature flags

The `systemprompt` facade exposes additive Cargo features. The default is `core`.
Disable defaults with `default-features = false` when selecting an independent feature.

```toml
[dependencies]
systemprompt = { version = "0.87", features = ["api"] }
```

## Supported compositions

A composition is a feature set of the facade. The supported ones are listed in `scripts/build-compositions.txt` of the source repository, and CI builds every one of them on each change; the two that link the `systemprompt` binary are also booted (`systemprompt --help` declares every member and builds the command tree):

| Composition | Features | Booted |
|---|---|---|
| reference | `reference` | yes |
| reference with Slack and Teams | `reference`, `slack`, `teams` | yes |
| API | `api` | no |
| CLI | `core`, `cli` | no |
| gateway | `api`, `ai` | no |
| minimal kernel | `core`, `database` | no |
| extension authoring | `core` | no |

Any other combination of features is unsupported: it may compile, but it is not built or tested.

## Feature matrix

Direct dependencies and feature implications from [the facade manifest](../../systemprompt/Cargo.toml).
Cargo enables the transitive dependencies of each selected feature.

| Feature | Direct dependencies and feature implications |
|---------|---------------------------------------------|
| `default` | `core` |
| `core` | `dep:systemprompt-traits`, `dep:systemprompt-models`, `dep:systemprompt-wire`, `dep:systemprompt-manifest`, `dep:systemprompt-identifiers`, `dep:systemprompt-extension`, `dep:systemprompt-template-provider` |
| `database` | `dep:systemprompt-database`, `dep:sqlx`, `systemprompt-extension?/sqlx` |
| `config` | `dep:systemprompt-config` |
| `mcp` | `dep:rmcp`, `systemprompt-models?/mcp` |
| `api` | `core`, `database`, `dep:systemprompt-api`, `dep:systemprompt-gateway`, `dep:systemprompt-runtime`, `dep:systemprompt-mcp-host`, `dep:systemprompt-oauth-issuance`, `dep:systemprompt-gateway-entry`, `dep:axum`, `systemprompt-extension?/router`, `systemprompt-slack-entry?/http`, `systemprompt-teams-entry?/http`, `systemprompt-agent-runs-entry?/http`, `systemprompt-notifications-entry?/http`, `systemprompt-content-entry?/http`, `systemprompt-analytics-entry?/http`, `systemprompt-marketplace-entry?/http`, `systemprompt-files-entry?/http`, `systemprompt-mcp-entry?/http`, `systemprompt-cloud-entry?/http`, `systemprompt-agent-entry?/http`, `systemprompt-brain-entry?/http`, `systemprompt-gateway-entry/http`, `systemprompt-identity-entry?/http` |
| `identity` | `dep:systemprompt-identity-entry` |
| `cloud` | `dep:systemprompt-cloud`, `dep:systemprompt-cloud-entry` |
| `logging` | `dep:systemprompt-logging` |
| `loader` | `dep:systemprompt-loader` |
| `events` | `dep:systemprompt-events` |
| `storage` | `dep:systemprompt-storage` |
| `client` | `dep:systemprompt-client` |
| `security` | `dep:systemprompt-security`, `dep:systemprompt-governance` |
| `cli` | `config`, `dep:systemprompt-cli`, `dep:systemprompt-extension`, `systemprompt-extension/cli`, `systemprompt-cloud-entry?/cli`, `systemprompt-content-entry?/cli`, `systemprompt-analytics-entry?/cli`, `systemprompt-marketplace-entry?/cli`, `systemprompt-files-entry?/cli`, `systemprompt-issues?/cli`, `systemprompt-mcp-entry?/cli`, `systemprompt-agent-entry?/cli`, `systemprompt-identity-entry?/cli` |
| `runtime` | `cli`, `dep:systemprompt-extension` |
| `analytics` | `dep:systemprompt-analytics`, `dep:systemprompt-analytics-entry` |
| `content` | `dep:systemprompt-content`, `dep:systemprompt-content-entry`, `systemprompt-analytics-entry?/content-commands` |
| `marketplace` | `dep:systemprompt-marketplace`, `dep:systemprompt-marketplace-entry` |
| `files` | `dep:systemprompt-files`, `dep:systemprompt-files-entry`, `dep:systemprompt-documents` |
| `mcp-servers` | `dep:systemprompt-mcp`, `dep:systemprompt-mcp-entry` |
| `agents` | `dep:systemprompt-agent`, `dep:systemprompt-agent-entry` |
| `slack` | `dep:systemprompt-slack`, `dep:systemprompt-slack-entry`, `systemprompt-extension?/slack` |
| `teams` | `dep:systemprompt-teams`, `dep:systemprompt-teams-entry`, `systemprompt-extension?/teams` |
| `agent-runs` | `dep:systemprompt-agent-runs`, `dep:systemprompt-agent-runs-entry` |
| `otel` | `systemprompt-logging?/otel`, `systemprompt-api?/otel`, `systemprompt-gateway-entry?/otel` |
| `bridge` | `systemprompt-api?/bridge` |
| `files-office` | `dep:systemprompt-files`, `systemprompt-files/office` |
| `files-pdf` | `dep:systemprompt-files`, `systemprompt-files/pdf` |
| `files-collab` | `dep:systemprompt-files`, `systemprompt-files/collab`, `dep:systemprompt-documents`, `systemprompt-documents/collab`, `dep:systemprompt-files-entry`, `systemprompt-files-entry/collab` |
| `files-collab-loro` | `files-collab`, `systemprompt-documents/engine-loro` |
| `notifications` | `dep:systemprompt-notifications`, `dep:systemprompt-notifications-entry`, `systemprompt-documents?/notifications` |
| `brain` | `dep:systemprompt-brain`, `dep:systemprompt-brain-entry`, `notifications` |
| `issues` | `dep:systemprompt-issues` |
| `ai` | `dep:systemprompt-ai`, `systemprompt-models?/mcp` |
| `reference` | `api`, `mcp`, `cloud`, `cli`, `agents`, `ai`, `mcp-servers`, `dep:systemprompt-oauth`, `dep:systemprompt-users`, `content`, `analytics`, `marketplace`, `dep:systemprompt-scheduler`, `dep:systemprompt-generator`, `logging`, `systemprompt-logging/cli`, `config`, `files`, `files-office`, `files-pdf`, `files-collab-loro`, `agent-runs`, `otel`, `bridge`, `notifications`, `brain`, `issues`, `identity`, `loader`, `events`, `storage`, `client`, `security` |

`dep:` selects an optional dependency; an unprefixed name selects another feature.
`dependency/feature` enables a feature on that dependency. `reference` excludes the opt-in
`slack` and `teams` integrations. Enable them explicitly when required.

`files-office` adds the Word, Excel, PowerPoint and OpenDocument codecs to the document store; the HTML, Markdown and text codecs and Markdown export are always present. `files-pdf` adds PDF export and its embedded fonts; without it a PDF export is refused with `unsupported_op`. `files-collab` mounts the live co-editing routes and builds the `CollabService`; without an engine every session route answers `503 collab_engine_unavailable`. `files-collab-loro` adds the Loro engine (`systemprompt-files/collab-loro`), the only engine core ships. The document-scoped live channel and awareness route are mounted with the store itself and need no collab feature.

`slack` and `teams` link the Slack and Teams entry crates, whose members contribute the `/api/v1/slack` and `/api/v1/teams` inbound routes; each route verifies its platform's signature or bot token in the handler, and a profile that disables the `slack` or `teams` member leaves its routes unmounted. `agent-runs` mounts `/api/v1/agent-runs` and wires the run launcher into hosted agents. `otel` mounts the gateway's OTLP/HTTP ingest routes and enables the live OTLP export of the server's own logs and spans. `bridge` mounts the device-credential consumer routes the bridge reports installation evidence through. Without a feature its member is not in `reference()`: its routes are not mounted and its job does not exist.

`notifications` adds notification streams, rules, the per-user feed and their outbox consumer. `brain` adds the brain: its capture consumer, gateway recall, routes and jobs (see [the brain](../brain.md)); it implies `notifications`, and a profile with `brain.enabled: false` composes none of it.

The `mcp` feature exposes `rmcp` and the `rmcp`-typed items of `systemprompt-models` (its `mcp` feature); the `systemprompt-mcp` domain crate is included by
`reference`. `ai` adds `systemprompt-ai`, the provider dispatch and tool execution, and is part of `reference`. `runtime` enables `cli` and the extension framework. `Composition` and `reference()`, which build a binary, need `reference`; `reference()` holds the core members of the features the build enables. The `api` dependency on
`systemprompt-runtime` enables its `geolocation` feature.

## Panic strategy

A build with `panic = "abort"` ends the process on any panic. A build with unwinding contains a panic to the job, background task or request that raised it; a request that panics receives the standard `500` error envelope and the panic message is logged.

## Examples and API documentation

The bundled `extension` and `cli` examples require `reference`; the `database` and `api` examples require
`database` and `api`. Published docs.rs documentation enables all features,
including integrations excluded from `reference`.

See [extensions](../concepts/extensions.md) for the composition contract and
[HTTP API](http-api.md) for server routes.
