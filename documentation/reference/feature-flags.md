# Facade feature flags

The `systemprompt` facade exposes additive Cargo features. The default is `core`.
Disable defaults with `default-features = false` when selecting an independent feature.

```toml
[dependencies]
systemprompt = { version = "0.87", features = ["api"] }
```

## Supported compositions

A composition is a feature set of the facade. The supported ones are listed in `scripts/build-compositions.txt` of the source repository, and CI builds every one of them on each change; those that link the `systemprompt` binary are also booted (`systemprompt --help` declares every member and builds the command tree):

| Composition | Features | Booted |
|---|---|---|
| reference | `reference` | yes |
| reference with Slack and Teams | `reference`, `slack`, `teams` | yes |
| runtime (the kernel alone) | `core`, `runtime` | yes |
| gateway flavour | `core`, `runtime`, `identity`, `ai`, `analytics`, `cloud`, `otel`, `bridge` | yes |
| MCP flavour | the gateway flavour, `mcp-servers`, `marketplace` | yes |
| agents flavour | the MCP flavour, `agents`, `agent-runs`, `notifications`, `brain` | yes |
| site flavour | `core`, `runtime`, `identity`, `analytics`, `content`, `files`, `files-pdf` | yes |
| API | `api` | no |
| gateway library | `api`, `ai` | no |
| minimal kernel | `core`, `database` | no |
| extension authoring | `core` | no |

A binary needs `runtime`: `Composition`, `reference()` and the `systemprompt` binary are gated on it. `reference()` holds every core member the build links, so a composition is chosen by its features alone.

### The kernel floor

`runtime` always links the crates below; no feature removes them. Their members are in every composition's `reference()`, and a member marked REQUIRED cannot be disabled in the profile when it is linked (the profile is refused with `RequiredDisabled`):

| Crate | Member | Standing |
|---|---|---|
| `systemprompt-database` | `database` | optional |
| `systemprompt-events` | `events` | optional |
| `systemprompt-users` | `users` | REQUIRED |
| `systemprompt-logging` | `logging` | REQUIRED |
| `systemprompt-governance` | `authz` | REQUIRED |
| `systemprompt-oauth` | `oauth` | optional |
| `systemprompt-oauth-issuance` | `identity` | optional |
| `systemprompt-mcp-host` | `mcp_host` | optional |
| `systemprompt-scheduler` | `scheduler` | optional |
| `systemprompt-api`, `systemprompt-runtime`, `systemprompt-cli`, `systemprompt-config`, `systemprompt-security` | (none) | the server, the application context and the CLI |

The floor links no capability crate: CI checks that the resolved dependency closure of the `runtime` composition holds none, so a capability whose feature is off contributes neither members, routes and commands nor code to the binary. The document store, files, the AI gateway and every other capability arrive with their own features (`files`, `ai`, ...), and `api` mounts each one's HTTP routes only when that feature is enabled.

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
| `api` | `core`, `database`, `dep:systemprompt-api`, `dep:systemprompt-runtime`, `dep:systemprompt-mcp-host`, `dep:systemprompt-oauth-issuance`, `dep:systemprompt-users`, `dep:systemprompt-oauth`, `dep:systemprompt-scheduler`, `dep:axum`, `systemprompt-extension?/router`, `systemprompt-slack-entry?/http`, `systemprompt-teams-entry?/http`, `systemprompt-agent-runs-entry?/http`, `systemprompt-notifications-entry?/http`, `systemprompt-content-entry?/http`, `systemprompt-analytics-entry?/http`, `systemprompt-marketplace-entry?/http`, `systemprompt-files-entry?/http`, `systemprompt-mcp-entry?/http`, `systemprompt-cloud-entry?/http`, `systemprompt-agent-entry?/http`, `systemprompt-brain-entry?/http`, `systemprompt-gateway-entry?/http`, `systemprompt-documents?/http`, `systemprompt-identity-entry?/http` |
| `identity` | `dep:systemprompt-identity-entry` |
| `cloud` | `dep:systemprompt-cloud`, `dep:systemprompt-cloud-entry` |
| `logging` | `dep:systemprompt-logging` |
| `loader` | `dep:systemprompt-loader` |
| `events` | `dep:systemprompt-events` |
| `storage` | `dep:systemprompt-storage` |
| `client` | `dep:systemprompt-client` |
| `security` | `dep:systemprompt-security`, `dep:systemprompt-governance` |
| `cli` | `config`, `dep:systemprompt-cli`, `dep:systemprompt-extension`, `systemprompt-extension/cli`, `systemprompt-cloud-entry?/cli`, `systemprompt-content-entry?/cli`, `systemprompt-analytics-entry?/cli`, `systemprompt-marketplace-entry?/cli`, `systemprompt-files-entry?/cli`, `systemprompt-issues?/cli`, `systemprompt-mcp-entry?/cli`, `systemprompt-agent-entry?/cli`, `systemprompt-identity-entry?/cli` |
| `runtime` | `api`, `cli`, `logging`, `systemprompt-logging/cli`, `security`, `events`, `config` |
| `analytics` | `dep:systemprompt-analytics`, `dep:systemprompt-analytics-entry` |
| `content` | `dep:systemprompt-content`, `dep:systemprompt-content-entry`, `dep:systemprompt-generator`, `systemprompt-analytics-entry?/content-commands` |
| `marketplace` | `dep:systemprompt-marketplace`, `dep:systemprompt-marketplace-entry` |
| `files` | `dep:systemprompt-files`, `dep:systemprompt-files-entry`, `dep:systemprompt-documents`, `dep:systemprompt-issues` |
| `mcp-servers` | `ai`, `dep:systemprompt-mcp`, `dep:systemprompt-mcp-entry` |
| `agents` | `ai`, `mcp-servers`, `dep:systemprompt-agent`, `dep:systemprompt-agent-entry` |
| `slack` | `dep:systemprompt-slack`, `dep:systemprompt-slack-entry`, `systemprompt-extension?/slack` |
| `teams` | `dep:systemprompt-teams`, `dep:systemprompt-teams-entry`, `systemprompt-extension?/teams` |
| `agent-runs` | `dep:systemprompt-agent-runs`, `dep:systemprompt-agent-runs-entry` |
| `otel` | `systemprompt-logging?/otel`, `systemprompt-api?/otel`, `systemprompt-gateway-entry?/otel` |
| `bridge` | `systemprompt-api?/bridge` |
| `files-office` | `dep:systemprompt-files`, `systemprompt-files/office` |
| `files-pdf` | `dep:systemprompt-files`, `systemprompt-files/pdf` |
| `files-collab` | `dep:systemprompt-files`, `systemprompt-files/collab`, `dep:systemprompt-documents`, `systemprompt-documents/collab`, `dep:systemprompt-files-entry`, `systemprompt-files-entry/collab` |
| `files-collab-loro` | `files-collab`, `systemprompt-documents/engine-loro` |
| `notifications` | `dep:systemprompt-notifications`, `dep:systemprompt-notifications-entry` |
| `brain` | `dep:systemprompt-brain`, `dep:systemprompt-brain-entry`, `notifications` |
| `ai` | `dep:systemprompt-ai`, `dep:systemprompt-gateway`, `dep:systemprompt-gateway-entry`, `systemprompt-models?/mcp` |
| `reference` | `runtime`, `mcp`, `cloud`, `agents`, `ai`, `mcp-servers`, `content`, `analytics`, `marketplace`, `files`, `files-office`, `files-pdf`, `files-collab-loro`, `agent-runs`, `otel`, `bridge`, `notifications`, `brain`, `identity`, `loader`, `storage`, `client` |

`dep:` selects an optional dependency; an unprefixed name selects another feature.
`dependency/feature` enables a feature on that dependency. `reference` excludes the opt-in
`slack` and `teams` integrations. Enable them explicitly when required.

`files-office` adds the Word, Excel, PowerPoint and OpenDocument codecs to the document store; the HTML, Markdown and text codecs and Markdown export are always present. `files-pdf` adds PDF export and its embedded fonts; without it a PDF export is refused with `unsupported_op`. `files-collab` mounts the live co-editing routes and builds the `CollabService`; without an engine every session route answers `503 collab_engine_unavailable`. `files-collab-loro` adds the Loro engine (`systemprompt-files/collab-loro`), the only engine core ships. The document-scoped live channel and awareness route are mounted with the store itself and need no collab feature.

`slack` and `teams` link the Slack and Teams entry crates, whose members contribute the `/api/v1/slack` and `/api/v1/teams` inbound routes; each route verifies its platform's signature or bot token in the handler, and a profile that disables the `slack` or `teams` member leaves its routes unmounted. `agent-runs` mounts `/api/v1/agent-runs` and wires the run launcher into hosted agents. `otel` mounts the gateway's OTLP/HTTP ingest routes and enables the live OTLP export of the server's own logs and spans. `bridge` mounts the device-credential consumer routes the bridge reports installation evidence through. Without a feature its member is not in `reference()`: its routes are not mounted and its job does not exist.

`agents` serves agent processes only together with `api`, which enables the `agent-http` member and with it the hosted A2A server that every agent child process runs; with `agents` and no `api`, an agent child host refuses to serve and names `agent-http`. `agent-runs` belongs to the agents capability and issue reports (`systemprompt-issues`) come with `files`, and `ai` holds the gateway's storage: each reaches no other capability except through the shared contracts the members provide.

`notifications` adds notification streams, rules, the per-user feed and their outbox consumer. `brain` adds the brain: its capture consumer, gateway recall, routes and jobs (see [the brain](../brain.md)); it implies `notifications`, and a profile with `brain.enabled: false` composes none of it.

The `mcp` feature exposes `rmcp` and the `rmcp`-typed items of `systemprompt-models` (its `mcp` feature); the `systemprompt-mcp` domain crate, its `mcp` and `mcp-http` members and its re-export in `systemprompt::mcp` come with `mcp-servers`. `ai` adds the `ai` member, the provider dispatch and tool execution, and the AI gateway (the `gateway` and `gateway-http` members), and is part of `reference`; `mcp-servers` and `agents` imply it. `runtime` is the kernel: `api`, `cli` and the infrastructure the kernel members need. `Composition` and `reference()`, which build a binary, need `runtime`; `reference()` holds the core members of the features the build enables. Without `content`, analytics events are recorded with no content id. The `api` dependency on
`systemprompt-runtime` enables its `geolocation` feature.

## Panic strategy

A build with `panic = "abort"` ends the process on any panic. A build with unwinding contains a panic to the job, background task or request that raised it; a request that panics receives the standard `500` error envelope and the panic message is logged.

## Examples and API documentation

The bundled `extension` example requires `reference` and the `cli` example `runtime`; the `database` and `api` examples require
`database` and `api`. Published docs.rs documentation enables all features,
including integrations excluded from `reference`.

See [extensions](../concepts/extensions.md) for the composition contract and
[HTTP API](http-api.md) for server routes.
