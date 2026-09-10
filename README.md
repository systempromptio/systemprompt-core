<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://systemprompt.io/files/images/logo.svg">
  <source media="(prefers-color-scheme: light)" srcset="https://systemprompt.io/files/images/logo-dark.svg">
  <img src="https://systemprompt.io/files/images/logo-dark.svg" alt="systemprompt.io" width="400">
</picture>

# SystemPrompt Core

SystemPrompt Core is the Rust library for a control plane you operate: identity, model access, MCP tool execution, policy and audit, with your domain capabilities compiled alongside them.

[![Crates.io](https://img.shields.io/crates/v/systemprompt.svg?style=flat-square)](https://crates.io/crates/systemprompt)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt?style=flat-square)](https://docs.rs/systemprompt)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](LICENSE)

[Evaluate](https://github.com/systempromptio/systemprompt-template) · [API docs](https://docs.rs/systemprompt) · [Architecture](documentation/overview.md) · [Security](SECURITY.md)

</div>

## Capabilities

Connect supported AI clients to your gateway and expose governed MCP tools. Core binds requests to identity, evaluates access and records decisions and usage in PostgreSQL. You operate the runtime, choose the upstream providers and retain the audit data.

| Capability | Interface |
|---|---|
| Identity | OAuth2/OIDC, authenticated sessions and access rules for users and resources. |
| Model gateway | Anthropic-compatible `/v1/messages`, model discovery and configurable upstream routing. |
| MCP and agents | Governed tool access and hosted agents with A2A transport. |
| Enforcement | Scope checks, credential-pattern detection, blocklists and rate limits on governed tool calls. |
| Audit | Correlated request and tool records linking identity, decisions, results and inference usage. |
| Composition | Your routes, providers, tools, jobs and persistence registered through extension contracts. |

Governance applies to traffic routed through these surfaces. Client-side tool execution needs the corresponding integration; a model gateway alone cannot govern every action on a laptop.

## Extension model

```text
SystemPrompt Core + your extensions + your configuration
                           ↓
                 Your compiled runtime
                           ↓
                      PostgreSQL
```

Build company-specific behavior on the same extension contract used by Core's own domains. The [AI domain](crates/domain/ai/src/extension.rs), for example, registers its schemas, migrations and dependencies through `Extension`.

An extension can contribute API routes, model and tool providers, scheduled jobs, configuration validation, schemas, migrations and roles. Gateway request guards let you add application-specific decisions before inference dispatch. The [demo's credit extension](https://github.com/systempromptio/systemprompt-demo/tree/next/extensions/credits) shows that hook in use.

Registration happens at compile/link time through `inventory`. At startup, the runtime discovers registrations and validates their dependency graph. See the [extension API source](crates/shared/extension/src/lib.rs) for the contract and registration flow.

Compiled extensions are trusted Rust code sharing your process. External MCP servers can run separately. This gives you a compact deployment while keeping process isolation available where the integration needs it.

## Use as a library

The workspace publishes a `systemprompt` facade with feature-gated `systemprompt-*` crates:

```toml
[dependencies]
systemprompt = { version = "0.50", features = ["full"] }
```

| Feature | Includes |
|---|---|
| `core` (default) | Shared traits, models, identifiers and extension contracts. |
| `database` | PostgreSQL integration. |
| `api` | HTTP server and application context. |
| `cli` | Command-line entry point. |
| `full` | Bundled runtime, domain modules and CLI; Slack and Teams remain opt-in. |

Use YAML to configure a deployment and Rust extensions to add behavior. Your host links those extensions and delegates startup to Core; the [template entry point](https://github.com/systempromptio/systemprompt-template/blob/next/src/main.rs) is a working example.

## Evaluate and build

For a running system with configuration, admin UI and scripted enforcement demos, start with the [MIT-licensed evaluation template](https://github.com/systempromptio/systemprompt-template).

To compile this workspace:

```bash
git clone https://github.com/systempromptio/systemprompt-core
cd systemprompt-core
just build-offline
```

Install Rust through rustup and [just](https://just.systems/). The repository pins its toolchain in [rust-toolchain.toml](rust-toolchain.toml) and declares Rust 1.96+ with edition 2024. `build-offline` uses the committed SQLx query cache without a running database; dependency downloads may still require network access. Runtime deployments require PostgreSQL 18+.

The runtime can operate inside a private network or air gap when models, tools and dependencies are available there. Requests routed to cloud providers leave that perimeter. Tool servers may introduce their own runtime dependencies.

## Evaluate the boundary

Tool credentials can be supplied to child-process environments without placing them in model context. Credential-pattern scanning provides an additional check on governed arguments; the tool process and its outputs remain trusted parts of the deployment.

Review the [evaluation documentation](documentation/), [production deployment guide](documentation/guides/deploy-production.md) and [template benchmarks](https://github.com/systempromptio/systemprompt-template/tree/next/demo/performance) against your own requirements. Report vulnerabilities through [SECURITY.md](SECURITY.md).

## License

Core is [BSL-1.1 source-available](LICENSE), available for evaluation, testing and non-production use under its license terms. Production use requires a commercial license. Each version converts to Apache-2.0 four years after publication.

You control your deployment and extensions; Core's license governs use of the underlying library. [Discuss production licensing](mailto:ed@systemprompt.io).
