# systemprompt-manifest

[![Crates.io](https://img.shields.io/crates/v/systemprompt-manifest.svg?style=flat-square)](https://crates.io/crates/systemprompt-manifest)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-manifest?style=flat-square)](https://docs.rs/systemprompt-manifest)
[![codecov](https://img.shields.io/codecov/c/github/systempromptio/systemprompt-core/main?style=flat-square&logo=codecov)](https://codecov.io/gh/systempromptio/systemprompt-core)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

Defines the services manifest, the on-disk profile, the secrets document and the global configuration assembled from them, together with the validators that run before the runtime starts.

**Layer**: Shared. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace. Depends on [`systemprompt-models`](https://crates.io/crates/systemprompt-models), [`systemprompt-wire`](https://crates.io/crates/systemprompt-wire), [`systemprompt-traits`](https://crates.io/crates/systemprompt-traits), [`systemprompt-identifiers`](https://crates.io/crates/systemprompt-identifiers) and [`systemprompt-provider-contracts`](https://crates.io/crates/systemprompt-provider-contracts).

## Installation

```toml
[dependencies]
systemprompt-manifest = "0.65"
```

## Module map

| Module | Contents |
|--------|----------|
| `services` | Services manifest: agents, plugins, hooks, MCP servers, skills, marketplaces, scheduler, provider registry, gateway policy, Slack and Teams apps, services bundles. |
| `validators` | Startup validation passes over the loaded configuration. |
| `profile` | On-disk profile: server, security, database, paths, cloud, rate limits, retention, observability. |
| `config` | Global `Config` singleton assembled from the profile and secrets. |
| `secrets` | Secrets document model and parsing. |
| `paths` | Path-resolution contract and well-known directory constants. |
| `env` | Environment-variable reading and `${VAR}` / `${VAR:-default}` interpolation. |
| `bridge_profile` | Builder of the `/v1/bridge/profile` payload from the provider registry and gateway policy. |

Errors are the `thiserror` enums of `systemprompt_models::errors` (`ServicesValidationError`, `GlobalConfigError`, `SecretsError`, …); `anyhow::Error` is never used in a public signature.

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial license. Each version converts to Apache 2.0 four years after publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).
