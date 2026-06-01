<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://systemprompt.io/files/images/logo.svg">
  <source media="(prefers-color-scheme: light)" srcset="https://systemprompt.io/files/images/logo-dark.svg">
  <img src="https://systemprompt.io/files/images/logo-dark.svg" alt="systemprompt.io" width="400">
</picture>

# systemprompt

The facade crate for systemprompt-core: a self-hosted platform for running AI agents and MCP servers under one governed boundary.

[![Crates.io](https://img.shields.io/crates/v/systemprompt.svg?style=flat-square)](https://crates.io/crates/systemprompt)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt?style=flat-square)](https://docs.rs/systemprompt)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)
[![Rust 1.85+](https://img.shields.io/badge/rust-1.85+-f97316?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![PostgreSQL 18+](https://img.shields.io/badge/postgres-18+-336791?style=flat-square&logo=postgresql&logoColor=white)](https://www.postgresql.org/)

</div>

> This README mirrors the [systemprompt-core root README](https://github.com/systempromptio/systemprompt-core#readme) and is published to [docs.rs](https://docs.rs/systemprompt). `systemprompt` re-exports the workspace crates behind feature flags; see the full project documentation in the [`documentation/`](https://github.com/systempromptio/systemprompt-core/tree/main/documentation) directory.

---

systemprompt-core compiles to a single Rust binary that you run on your own infrastructure, backed by a PostgreSQL database you own. It hosts AI agents (A2A protocol), MCP servers, an OAuth2/OIDC authorization server, and a provider gateway behind one HTTP surface. Every request passes through one authenticated, authorized, and audited path. The binary holds no durable state and makes no outbound calls for governance operation; PostgreSQL is the only state, and secrets stay under your own key-management lifecycle.

## Capabilities

| Capability | What it provides |
|------------|------------------|
| **A2A agents** | A standalone agent server speaking the agent-to-agent JSON-RPC protocol with SSE streaming and `.well-known` discovery. |
| **MCP servers** | Model Context Protocol servers hosted natively over streamable HTTP, each with scoped tools, OAuth2, and an access log. |
| **OAuth2 / OIDC** | A built-in authorization server with OIDC discovery, PKCE (S256), and WebAuthn. JWTs are RS256. |
| **Provider gateway** | A `/v1` proxy (`POST /v1/messages`, `GET /v1/models`) that routes model patterns to a configured upstream provider. |
| **Extensions** | Compile-time `Extension` implementations registered with the `inventory` crate. No runtime plugin loading. |
| **Governance** | Fail-closed (default-deny) authorization hook, rate limiting, and structured audit logging correlated by `trace_id`. |

## Requirements

- Rust 1.85+ (the workspace is edition 2024).
- PostgreSQL 18+.

## Use as a library

```toml
[dependencies]
systemprompt = { version = "0.14.0", features = ["full"] }
```

```rust
use systemprompt::prelude::*;
```

| Feature | Includes |
|---------|----------|
| `core` *(default)* | traits, models, identifiers, extension |
| `database` | PostgreSQL abstraction (`DbPool`) |
| `api` | HTTP server and `AppContext` (requires `core` + `database`) |
| `cli` | CLI entry point |
| `full` | Everything: all domain modules + CLI |

## Quickstart (building from source)

```bash
git clone https://github.com/systempromptio/systemprompt-core.git
cd systemprompt-core
just build

./target/debug/systemprompt admin setup --environment local --migrate --yes
./target/debug/systemprompt infra services start --api
```

```bash
curl -i http://127.0.0.1:8080/health   # 200 when the process and database are up
curl -s http://127.0.0.1:8080/api/v1   # discovery document of mounted surfaces
```

The full walkthrough is in [documentation/getting-started.md](https://github.com/systempromptio/systemprompt-core/blob/main/documentation/getting-started.md).

## License

Business Source License 1.1 (BSL-1.1). Source-available for evaluation, testing, and non-production use; production use requires a commercial license. Each version converts to Apache-2.0 four years after its publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE). Licensing enquiries: [ed@systemprompt.io](mailto:ed@systemprompt.io).

## Security

Report vulnerabilities to **ed@systemprompt.io**, not via public issues. See [SECURITY.md](https://github.com/systempromptio/systemprompt-core/blob/main/SECURITY.md).
