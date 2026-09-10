# Facade feature flags

The `systemprompt` facade exposes additive Cargo features. The default is `core`.
Disable defaults with `default-features = false` when selecting an independent feature.

```toml
[dependencies]
systemprompt = { version = "0.49", features = ["api"] }
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
| `api` | `core`, `database`, `dep:systemprompt-api`, `dep:systemprompt-runtime`, `dep:axum` |
| `cloud` | `dep:systemprompt-cloud` |
| `logging` | `dep:systemprompt-logging` |
| `loader` | `dep:systemprompt-loader` |
| `events` | `dep:systemprompt-events` |
| `storage` | `dep:systemprompt-storage` |
| `client` | `dep:systemprompt-client` |
| `security` | `dep:systemprompt-security` |
| `cli` | `dep:systemprompt-cli` |
| `runtime` | `cli`, `dep:systemprompt-extension` |
| `evaluation` | `dep:systemprompt-evaluation` |
| `analytics` | `dep:systemprompt-analytics` |
| `slack` | `dep:systemprompt-slack` |
| `teams` | `dep:systemprompt-teams` |
| `full` | `api`, `mcp`, `cloud`, `cli`, `dep:systemprompt-agent`, `dep:systemprompt-ai`, `dep:systemprompt-mcp`, `dep:systemprompt-oauth`, `dep:systemprompt-users`, `dep:systemprompt-content`, `analytics`, `evaluation`, `dep:systemprompt-marketplace`, `dep:systemprompt-scheduler`, `dep:systemprompt-generator`, `logging`, `systemprompt-logging/cli`, `config`, `dep:systemprompt-files`, `loader`, `events`, `storage`, `client`, `security` |

`dep:` selects an optional dependency; an unprefixed name selects another feature.
`dependency/feature` enables a feature on that dependency. `full` excludes the opt-in
`slack` and `teams` integrations. Enable them explicitly when required.

The `mcp` feature exposes `rmcp`; the `systemprompt-mcp` domain crate is included by
`full`. `runtime` enables the CLI-backed runtime builder. The `api` dependency on
`systemprompt-runtime` enables its `geolocation` feature.

## Examples and API documentation

The bundled `extension`, `database`, `api` and `cli` examples require `core`, `database`,
`api` and `cli`, respectively. Published docs.rs documentation enables all features,
including integrations excluded from `full`.

See [extensions](../concepts/extensions.md) for registration contracts and
[HTTP API](http-api.md) for server routes.
