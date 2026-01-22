# systemprompt-identifiers

Core identifier types for systemprompt.io OS.

[![Crates.io](https://img.shields.io/crates/v/systemprompt-identifiers.svg)](https://crates.io/crates/systemprompt-identifiers)
[![Documentation](https://docs.rs/systemprompt-identifiers/badge.svg)](https://docs.rs/systemprompt-identifiers)
[![License: FSL-1.1-ALv2](https://img.shields.io/badge/License-FSL--1.1--ALv2-blue.svg)](LICENSE)

## Overview

**Part of the Shared layer in the systemprompt.io architecture.**

Provides strongly-typed wrappers for all domain identifiers, ensuring type safety
and preventing accidental mixing of different ID types.

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
systemprompt-identifiers = "0.0.1"
```

## Quick Example

```rust
use systemprompt_identifiers::{UserId, TaskId, ContextId};

let user_id = UserId::new();
let task_id = TaskId::new();
let context_id = ContextId::new();

println!("User: {}, Task: {}, Context: {}", user_id, task_id, context_id);
```

## Types

- `SessionId` - User session identifier
- `UserId` - User identifier
- `AgentId` - Agent UUID identifier
- `AgentName` - Agent name string
- `TaskId` - Task identifier
- `ContextId` - Conversation context identifier
- `TraceId` - Distributed tracing identifier
- `ClientId` - OAuth client identifier
- `McpExecutionId` - MCP execution tracking ID
- `McpServerId` - MCP server name
- `SkillId` - Skill identifier
- `SourceId` - Content source identifier
- `CategoryId` - Content category identifier
- `JwtToken` - JWT token wrapper

## Usage

All ID types implement:
- `Clone`, `Debug`, `PartialEq`, `Eq`, `Hash`
- `Serialize`, `Deserialize` (with `#[serde(transparent)]`)
- `AsRef<str>`, `Display`

## Feature Flags

| Feature | Default | Description |
|---------|---------|-------------|
| `sqlx` | No | SQLx type implementations for database queries |

## Dependencies

- `serde` - Serialization
- `uuid` - UUID generation
- `schemars` - JSON schema generation

## License

FSL-1.1-ALv2 - See [LICENSE](../../LICENSE) for details.
