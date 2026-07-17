<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://systemprompt.io/files/images/logo.svg">
  <source media="(prefers-color-scheme: light)" srcset="https://systemprompt.io/files/images/logo-dark.svg">
  <img src="https://systemprompt.io/files/images/logo.svg" alt="systemprompt.io" width="180">
</picture>

### Typed identity for every boundary you audit

[**Website**](https://systemprompt.io) · [**Documentation**](https://systemprompt.io/documentation/) · [**Guides**](https://systemprompt.io/guides) · [**Core**](https://github.com/systempromptio/systemprompt-core) · [**Template**](https://github.com/systempromptio/systemprompt-template) · [**Discord**](https://discord.gg/wkAbSuPWpr)

</div>

---

# systemprompt-identifiers

<div align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/dark/00-overview.svg">
    <source media="(prefers-color-scheme: light)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/light/00-overview.svg">
    <img alt="systemprompt-identifiers — systemprompt-core workspace" src="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/dark/00-overview.svg" width="100%">
  </picture>
</div>

[![Crates.io](https://img.shields.io/crates/v/systemprompt-identifiers.svg?style=flat-square)](https://crates.io/crates/systemprompt-identifiers)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-identifiers?style=flat-square)](https://docs.rs/systemprompt-identifiers)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)
[![codecov](https://img.shields.io/codecov/c/github/systempromptio/systemprompt-core/main?style=flat-square&logo=codecov)](https://codecov.io/gh/systempromptio/systemprompt-core)

Every ID in systemprompt.io carries its type. A `UserId` cannot stand in for an `AgentId`, and the compiler proves it before a single request reaches the audit trail. This crate holds the newtype identifiers (`UserId`, `TraceId`, `AgentId`, `McpServerId`, and the rest) that name each boundary in the governance pipeline.

**Layer**: Shared. Foundational types with no dependencies on other layers. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## What it guarantees

Distinct ID types never mix at a call site. Passing a `UserId` where an `AgentId` is expected is a compile error, not a runtime surprise found in a log. Validated identifiers reject malformed input at construction, so a bad email or path fails at the edge rather than deep in a query.

## Layout

| Module | Contents |
|--------|----------|
| `macros/` | `define_id!` / `define_token!` and shared construction helpers |
| `db_value/` | `DbValue` enum, `ToDbValue` / `FromDbValue` traits, `JsonRow` |
| `auth/` | `ApiKeyId`, `ApiKeySecret`, `CloudAuthToken`, `DeviceCertId`, `JwtToken`, `SessionToken` |
| `error` | `IdValidationError` |
| `headers` | HTTP header name constants |
| `agent`, `ai`, `mcp` | Agent, AI gateway, and MCP identifiers (`AgentId`, `AgentName`, `McpServerId`, `McpToolName`, `AiRequestId`, `MessageId`) |
| `oauth`, `client`, `session`, `connection` | Auth-flow and session identity (`AccessTokenId`, `ClientId`, `SessionId`, `ConnectionId`) |
| `content`, `execution`, `task`, `hook`, `section` | Content, task, and execution-step identifiers |
| `cloud`, `tenant`, `teams`, `marketplace`, `plugin` | Cloud, tenancy, and distribution identifiers |
| `funnel`, `links`, `events`, `slack` | Analytics, link, and integration identifiers |
| `trace`, `context`, `gateway_conversation`, `gateway_boot`, `provider_request` | Request-tracing and gateway correlation identifiers |
| `user`, `actor`, `roles`, `policy` | Principal and authorization identifiers |
| `email`, `profile`, `url`, `path`, `locale` | Validated value types (`Email`, `ProfileName`, `ValidatedUrl`, `ValidatedFilePath`, `LocaleCode`) |
| `jobs`, `webhook` | Job and webhook identifiers |

Per-type detail lives on [docs.rs](https://docs.rs/systemprompt-identifiers).

## Usage

```toml
[dependencies]
systemprompt-identifiers = "0.21"
```

```rust
use systemprompt_identifiers::{AgentId, TaskId, UserId};

// Known string value (literal, parsed input, DB row).
let user = UserId::new("user_42");
let agent = AgentId::new("developer_agent");

// Mint a fresh UUID-backed identifier.
let task = TaskId::generate();

// Mixing newtype IDs is a compile error.
// let broken: AgentId = user; // error[E0308]: mismatched types

println!("agent={agent}, user={user}, task={task}");
```

Validated identifiers (`Email`, `ProfileName`, `ValidatedUrl`, `ValidatedFilePath`, `AgentName`, `McpServerId`) expose a fallible constructor:

```rust
use systemprompt_identifiers::Email;
let email = Email::try_new("alice@example.com")?;
```

## Traits Implemented

All ID types implement `Clone`, `Debug`, `PartialEq`, `Eq`, `Hash`, `Serialize`, `Deserialize` (`#[serde(transparent)]`), `AsRef<str>`, and `Display`. With the `sqlx` feature, every identifier also derives `sqlx::Type` for direct binding in `query_as!` macros.

## Feature Flags

| Feature | Default | Description |
|---------|---------|-------------|
| `sqlx` | off | Derives `sqlx::Type` on every identifier for database binding. |

## Dependencies

- `serde`, `serde_json`: serialisation
- `uuid`: UUID generation
- `schemars`: JSON schema generation
- `chrono`: timestamps on `DbValue`
- `thiserror`: `IdValidationError`
- `sqlx` (optional): database type derivation

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial license. Each version converts to Apache 2.0 four years after publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).

---

<div align="center">

**[systemprompt.io](https://systemprompt.io)** · **[Documentation](https://systemprompt.io/documentation/)** · **[Guides](https://systemprompt.io/guides)** · **[Live Demo](https://systemprompt.io/features/demo)** · **[Template](https://github.com/systempromptio/systemprompt-template)** · **[crates.io](https://crates.io/crates/systemprompt-identifiers)** · **[docs.rs](https://docs.rs/systemprompt-identifiers)** · **[Discord](https://discord.gg/wkAbSuPWpr)**

<sub>Shared layer · Own how your organization uses AI.</sub>

</div>
