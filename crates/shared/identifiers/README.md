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

Strongly-typed newtype wrappers for every domain identifier in systemprompt.io. Distinct ID types cannot be mixed at call sites; the compiler rejects passing a `UserId` where an `AgentId` is expected.

## Layout

```
src/
├── lib.rs                  // crate root, re-exports
├── macros/                 // define_id! / define_token! and helpers
│   ├── id.rs
│   ├── token.rs
│   ├── helpers.rs
│   └── mod.rs
├── db_value/               // database boundary types
│   ├── value.rs            // DbValue enum
│   ├── to_value.rs         // ToDbValue trait
│   ├── from_value.rs       // FromDbValue trait + JsonRow
│   └── mod.rs
├── auth/                   // ApiKeyId, ApiKeySecret, CloudAuthToken,
│   │                       // DeviceCertId, JwtToken, SessionToken
│   └── …
├── error.rs                // IdValidationError
├── headers.rs              // HTTP header name constants
├── agent.rs                // AgentId, AgentName, ExternalAgentId
├── ai.rs                   // AiGatewayPolicyId, AiQuotaBucketId,
│                           // AiRequestId, AiSafetyFindingId,
│                           // ConfigId, MessageId
├── client.rs               // ClientId, ClientType
├── cloud.rs                // CheckoutSessionId, PriceId, TransactionId
├── connection.rs           // ConnectionId
├── content.rs              // CategoryId, ContentId, FileId, SkillId,
│                           // SourceId, TagId
├── context.rs              // ContextId
├── email.rs                // Email (validated)
├── execution.rs            // ArtifactId, ExecutionStepId, LogId, TokenId
├── funnel.rs               // EngagementEventId, FunnelId,
│                           // FunnelProgressId
├── gateway_conversation.rs // GatewayConversationId
├── hook.rs                 // HookId
├── jobs.rs                 // JobName, ScheduledJobId
├── links.rs                // CampaignId, LinkClickId, LinkId
├── locale.rs               // LocaleCode
├── marketplace.rs          // MarketplaceId
├── mcp.rs                  // AiToolCallId, McpExecutionId, McpServerId
├── oauth.rs                // AccessTokenId, AuthorizationCode,
│                           // ChallengeId, RefreshTokenId
├── path.rs                 // ValidatedFilePath
├── plugin.rs               // PluginId
├── policy.rs               // PolicyVersion
├── profile.rs              // ProfileName (validated)
├── provider_request.rs     // ProviderRequestId
├── roles.rs                // RoleId
├── section.rs              // SectionId
├── session.rs              // SessionId, SessionSource
├── task.rs                 // TaskId
├── tenant.rs               // TenantId
├── trace.rs                // TraceId
├── url.rs                  // ValidatedUrl
├── user.rs                 // UserId
└── webhook.rs              // WebhookEndpointId
```

## Usage

```toml
[dependencies]
systemprompt-identifiers = "0.13.1"
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

- `serde`, `serde_json` — serialisation
- `uuid` — UUID generation
- `schemars` — JSON schema generation
- `chrono` — timestamps on `DbValue`
- `thiserror` — `IdValidationError`
- `sqlx` (optional) — database type derivation

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial license. Each version converts to Apache 2.0 four years after publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).

---

<div align="center">

**[systemprompt.io](https://systemprompt.io)** · **[Documentation](https://systemprompt.io/documentation/)** · **[Guides](https://systemprompt.io/guides)** · **[Live Demo](https://systemprompt.io/features/demo)** · **[Template](https://github.com/systempromptio/systemprompt-template)** · **[crates.io](https://crates.io/crates/systemprompt-identifiers)** · **[docs.rs](https://docs.rs/systemprompt-identifiers)** · **[Discord](https://discord.gg/wkAbSuPWpr)**

<sub>Shared layer · Own how your organization uses AI.</sub>

</div>
