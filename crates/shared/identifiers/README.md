# systemprompt-identifiers

Core identifier types for SystemPrompt OS.

## Purpose

Provides strongly-typed wrappers for all domain identifiers, ensuring type safety
and preventing accidental mixing of different ID types.

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
- `sqlx::Type`, `sqlx::Encode`, `sqlx::Decode`

## Dependencies

- `serde` - Serialization
- `uuid` - UUID generation
- `sqlx` - Database support
- `schemars` - JSON schema generation
