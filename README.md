<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://systemprompt.io/files/images/logo.svg">
  <source media="(prefers-color-scheme: light)" srcset="https://systemprompt.io/files/images/logo-dark.svg">
  <img src="https://systemprompt.io/files/images/logo-dark.svg" alt="systemprompt.io" width="400">
</picture>

# systemprompt-core

The governance engine behind the only AI infrastructure you actually own. One Rust binary, one PostgreSQL, every agent and tool call through one audited path.

[![Crates.io](https://img.shields.io/crates/v/systemprompt.svg?style=flat-square)](https://crates.io/crates/systemprompt)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt?style=flat-square)](https://docs.rs/systemprompt)
[![CI](https://img.shields.io/github/actions/workflow/status/systempromptio/systemprompt-core/ci.yml?branch=main&style=flat-square&label=CI)](https://github.com/systempromptio/systemprompt-core/actions/workflows/ci.yml)
[![codecov](https://img.shields.io/codecov/c/github/systempromptio/systemprompt-core/main?style=flat-square&logo=codecov)](https://codecov.io/gh/systempromptio/systemprompt-core)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](LICENSE)
[![Rust 1.94+](https://img.shields.io/badge/rust-1.94+-f97316?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![PostgreSQL 18+](https://img.shields.io/badge/postgres-18+-336791?style=flat-square&logo=postgresql&logoColor=white)](https://www.postgresql.org/)

[**Website**](https://systemprompt.io) · [**Documentation**](documentation/overview.md) · [**Evaluation template**](https://github.com/systempromptio/systemprompt-template) · [**Discord**](https://discord.gg/wkAbSuPWpr)

</div>

---

## Why this exists

Most teams govern AI one of two ways. They rent a dashboard, and someone else's infrastructure holds their prompts, their keys, and their audit trail. Or they build it themselves, and eighteen months later they are maintaining a distributed system instead of shipping.

This is the third option. systemprompt-core compiles to a single Rust binary you run on your own infrastructure, backed by the only state it has: a PostgreSQL database you own. Every agent, MCP tool call, and inference request passes through one authenticated, authorized, audited path: a synchronous in-process pipeline under 5 ms, writing an 18-column audit row for every decision. Credentials live in a ChaCha20-Poly1305 store and are injected only into tool child processes, never into the LLM context.

Zero outbound telemetry by default. Air-gap capable. Built for SOC 2 Type II, ISO 27001, HIPAA, and the OWASP Agentic Top 10.

<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-template/main/demo/recording/svg/output/dark/cap-secrets.svg">
  <source media="(prefers-color-scheme: light)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-template/main/demo/recording/svg/output/light/cap-secrets.svg">
  <img src="https://raw.githubusercontent.com/systempromptio/systemprompt-template/main/demo/recording/svg/output/dark/cap-secrets.svg" alt="An agent attempts to exfiltrate a GitHub PAT through a tool argument; the secret-detection layer denies the call before the tool process spawns" width="820">
</picture>

<sub>Live capture: an agent tries to pass a GitHub PAT through a tool argument and is denied before the tool process spawns. Evaluating? Start with the MIT-licensed <a href="https://github.com/systempromptio/systemprompt-template">systemprompt-template</a>: 43 scripted demos run every claim on your laptop.</sub>

</div>

## Capabilities

Six surfaces, one binary, one authenticated and audited path.

| Capability | What it provides |
|------------|------------------|
| **A2A agents** | A standalone agent server speaking the agent-to-agent JSON-RPC protocol, with SSE streaming and `.well-known` discovery. Agents you host, not seats you rent. |
| **MCP servers** | Model Context Protocol servers hosted natively over streamable HTTP. Each is an isolated OAuth2 resource server with scoped tools and its own access log. |
| **OAuth2 / OIDC** | A built-in authorization server: OIDC discovery, PKCE (S256), WebAuthn, RS256 JWTs. No external identity dependency required to run. |
| **Provider gateway** | A `/v1` proxy (`POST /v1/messages`, `GET /v1/models`) that routes model patterns to any configured upstream. Swap providers without touching clients. |
| **Extensions** | Compile-time `Extension` implementations registered with the `inventory` crate. Your code becomes part of your binary; no runtime plugin loading, nothing injected you didn't compile. |
| **Governance** | Fail-closed (default-deny) authorization, secret detection, rate limiting, and structured audit logging correlated by `trace_id`. Every deny reason is a queryable row. |

## Requirements

- Rust 1.94+ (the workspace is edition 2024; the repository pins a nightly toolchain in `rust-toolchain.toml`).
- PostgreSQL 18+.
- [`just`](https://just.systems) to run the build recipes.

## Quickstart

```bash
git clone https://github.com/systempromptio/systemprompt-core.git
cd systemprompt-core
just build

# Generate a profile + secrets, provision the database, and migrate
./target/debug/systemprompt admin setup --environment local --migrate --yes

# Start the API server (binds 127.0.0.1:8080 by default)
./target/debug/systemprompt infra services start --api
```

Confirm it is serving:

```bash
curl -i http://127.0.0.1:8080/health   # 200 when the process and database are up
curl -s http://127.0.0.1:8080/api/v1   # discovery document of mounted surfaces
```

The full walkthrough is in [documentation/getting-started.md](documentation/getting-started.md).

## Performance

Governance overhead benchmarked at 3,308 req/s burst with p99 latency of 22.7 ms: under 1% of AI response time. Reproduce it yourself with `just benchmark` in the [evaluation template](https://github.com/systempromptio/systemprompt-template).

## Use as a library

The workspace publishes to crates.io as `systemprompt-*` crates behind the `systemprompt` facade.

```toml
[dependencies]
systemprompt = { version = "0.23", features = ["full"] }
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

## Documentation

- [Overview](documentation/overview.md) — what the system is and when to use it.
- [Getting started](documentation/getting-started.md) — clean machine to a running server.
- [Deploy in production](documentation/guides/deploy-production.md) — HA, backup, key rotation, monitoring.
- The top level of [`documentation/`](documentation/) holds the security and procurement evaluation pack.

## License

Business Source License 1.1 (BSL-1.1). Source-available for evaluation, testing, and non-production use; production use requires a commercial license. Each version converts to Apache-2.0 four years after its publication. You will always be able to read, audit, and eventually own this code. See [LICENSE](LICENSE). Licensing enquiries: [ed@systemprompt.io](mailto:ed@systemprompt.io).

## Security

Report vulnerabilities to **ed@systemprompt.io**, not via public issues. See [SECURITY.md](SECURITY.md).

---

<div align="center">

**[systemprompt.io](https://systemprompt.io)** · **[Documentation](https://systemprompt.io/documentation/)** · **[Guides](https://systemprompt.io/guides)** · **[Live Demo](https://systemprompt.io/features/demo)** · **[Template](https://github.com/systempromptio/systemprompt-template)** · **[crates.io](https://crates.io/crates/systemprompt)** · **[docs.rs](https://docs.rs/systemprompt)** · **[Discord](https://discord.gg/wkAbSuPWpr)**

<sub>Rent your control plane and you rent your audit trail. This one compiles.</sub>
</div>
