<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://systemprompt.io/files/images/logo.svg">
  <source media="(prefers-color-scheme: light)" srcset="https://systemprompt.io/files/images/logo-dark.svg">
  <img src="https://systemprompt.io/files/images/logo.svg" alt="systemprompt.io" width="180">
</picture>

### Production infrastructure for AI agents

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

Typed newtype identifiers (`UserId`, `TraceId`, `AgentId`, `McpServerId`, and more) for systemprompt.io AI governance infrastructure. Enforces type-safe IDs across every boundary in the MCP governance pipeline, preventing accidental mixing of different ID types at compile time.

**Layer**: Shared — foundational types/traits with no dependencies on other layers. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Overview

Provides strongly-typed wrappers for all domain identifiers, ensuring type safety and preventing accidental mixing of different ID types.

## Architecture

- `SessionId` — User session identifier
- `UserId` — User identifier
- `AgentId` — Agent UUID identifier
- `AgentName` — Agent name string
- `TaskId` — Task identifier
- `ContextId` — Conversation context identifier
- `TraceId` — Distributed tracing identifier
- `ClientId` — OAuth client identifier
- `McpExecutionId` — MCP execution tracking ID
- `McpServerId` — MCP server name
- `SkillId` — Skill identifier
- `SourceId` — Content source identifier
- `CategoryId` — Content category identifier
- `JwtToken` — JWT token wrapper

## Usage

```toml
[dependencies]
systemprompt-identifiers = "0.2.1"
```

```rust
use systemprompt_identifiers::{UserId, TaskId, ContextId};

let user_id = UserId::new();
let task_id = TaskId::new();
let context_id = ContextId::new();

println!("User: {}, Task: {}, Context: {}", user_id, task_id, context_id);
```

```rust
use systemprompt_identifiers::{AgentId, UserId};

fn main() {
    let agent = AgentId::new("developer_agent");
    let user = UserId::new("user_42");

    // Mixing newtype IDs is a compile error — the types are distinct.
    // let broken: AgentId = user; // error[E0308]: mismatched types

    println!("agent = {agent}, user = {user}");
}
```

## Types

All ID types implement:
- `Clone`, `Debug`, `PartialEq`, `Eq`, `Hash`
- `Serialize`, `Deserialize` (with `#[serde(transparent)]`)
- `AsRef<str>`, `Display`

## Feature Flags

| Feature | Default | Description |
|---------|---------|-------------|
| `sqlx` | No | SQLx type implementations for database queries |

## Dependencies

- `serde` — Serialization
- `uuid` — UUID generation
- `schemars` — JSON schema generation

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial license. Each version converts to Apache 2.0 four years after publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).

---

<div align="center">

**[systemprompt.io](https://systemprompt.io)** · **[Documentation](https://systemprompt.io/documentation/)** · **[Guides](https://systemprompt.io/guides)** · **[Live Demo](https://systemprompt.io/features/demo)** · **[Template](https://github.com/systempromptio/systemprompt-template)** · **[crates.io](https://crates.io/crates/systemprompt-identifiers)** · **[docs.rs](https://docs.rs/systemprompt-identifiers)** · **[Discord](https://discord.gg/wkAbSuPWpr)**

<sub>Shared layer · Own how your organization uses AI.</sub>

</div>
