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

# systemprompt-events

<div align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/dark/00-overview.svg">
    <source media="(prefers-color-scheme: light)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/light/00-overview.svg">
    <img alt="systemprompt-events — systemprompt-core workspace" src="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/dark/00-overview.svg" width="100%">
  </picture>
</div>

[![Crates.io](https://img.shields.io/crates/v/systemprompt-events.svg?style=flat-square)](https://crates.io/crates/systemprompt-events)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-events?style=flat-square)](https://docs.rs/systemprompt-events)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

Event bus, SSE broadcasters, and fan-out routing for systemprompt.io AI governance infrastructure. A2A, analytics, and context stream wiring for the MCP governance pipeline. Manages connection lifecycles, routes events to appropriate channels, and handles automatic cleanup of disconnected clients.

**Layer**: Infra — infrastructure primitives (database, security, events, etc.) consumed by domain crates. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Overview

This crate provides a type-safe, generic event broadcasting system for real-time communication with connected clients via SSE (Server-Sent Events). It manages connection lifecycles, routes events to appropriate channels, and handles automatic cleanup of disconnected clients.

## Architecture

```
crates/infra/events/
├── Cargo.toml
├── README.md
├── status.md
└── src/
    ├── lib.rs                      # 27 lines  - Trait definitions, type aliases, re-exports
    └── services/
        ├── mod.rs                  # 10 lines  - Module re-exports
        ├── broadcaster.rs          # 191 lines - GenericBroadcaster implementation
        └── routing.rs              # 51 lines  - EventRouter, global singletons
```

### `lib.rs`
Entry point defining core abstractions:
- `Broadcaster` trait - Type-safe async broadcasting with connection management
- `EventSender` type alias - Channel sender for SSE events (`UnboundedSender<Result<Event, Infallible>>`)

### `services/broadcaster.rs`
Generic broadcaster implementation:
- `GenericBroadcaster<E>` - Thread-safe broadcaster using `Arc<RwLock<HashMap<UserId, HashMap<ConnId, Sender>>>>`
- `ConnectionGuard<E>` - RAII guard for automatic connection cleanup on drop
- Type aliases: `AgUiBroadcaster`, `A2ABroadcaster`, `ContextBroadcaster`, `AnalyticsBroadcaster`
- Keep-alive utilities: `standard_keep_alive()`, `HEARTBEAT_INTERVAL`, `HEARTBEAT_JSON`

### `services/routing.rs`
Event routing and global state:
- `EventRouter` - Routes events to appropriate broadcaster(s)
- Global singletons: `AGUI_BROADCASTER`, `A2A_BROADCASTER`, `CONTEXT_BROADCASTER`, `ANALYTICS_BROADCASTER`

### Event Flow

```
                    ┌─────────────────┐
                    │   EventRouter   │
                    └────────┬────────┘
                             │
        ┌────────────────────┼────────────────────┐
        │                    │                    │
        ▼                    ▼                    ▼
┌───────────────┐    ┌───────────────┐    ┌───────────────┐
│AGUI_BROADCASTER│    │A2A_BROADCASTER│    │CONTEXT_BROADCASTER│
└───────┬───────┘    └───────┬───────┘    └───────┬───────┘
        │                    │                    │
        ▼                    ▼                    ▼
   SSE Clients          SSE Clients          SSE Clients
```

AgUI and A2A events are routed to both their primary broadcaster AND the context broadcaster for aggregation.

## Usage

```toml
[dependencies]
systemprompt-events = "0.2.1"
```

```rust
use systemprompt_events::{AGUI_BROADCASTER, Broadcaster};
use systemprompt_identifiers::UserId;

async fn active_listeners(user_id: &UserId) -> usize {
    AGUI_BROADCASTER.connection_count(user_id).await
}
```

## Public API

### Traits
| Trait | Methods | Purpose |
|-------|---------|---------|
| `Broadcaster` | `register`, `unregister`, `broadcast`, `connection_count`, `total_connections` | Type-safe event broadcasting |

### Types
| Type | Description |
|------|-------------|
| `EventSender` | `UnboundedSender<Result<Event, Infallible>>` |
| `GenericBroadcaster<E>` | Generic broadcaster for any `ToSse + Clone + Send + Sync` event |
| `AgUiBroadcaster` | `GenericBroadcaster<AgUiEvent>` |
| `A2ABroadcaster` | `GenericBroadcaster<A2AEvent>` |
| `ContextBroadcaster` | `GenericBroadcaster<ContextEvent>` |
| `AnalyticsBroadcaster` | `GenericBroadcaster<AnalyticsEvent>` |
| `ConnectionGuard<E>` | RAII guard for automatic unregistration |
| `EventRouter` | Routes events to appropriate broadcasters |

### Constants
| Constant | Value | Purpose |
|----------|-------|---------|
| `HEARTBEAT_INTERVAL` | 15 seconds | SSE keep-alive interval |
| `HEARTBEAT_JSON` | `{"type":"heartbeat"}` | Keep-alive payload |

### Global Singletons
| Static | Type | Purpose |
|--------|------|---------|
| `AGUI_BROADCASTER` | `LazyLock<AgUiBroadcaster>` | AG-UI event broadcasts |
| `A2A_BROADCASTER` | `LazyLock<A2ABroadcaster>` | Agent-to-agent events |
| `CONTEXT_BROADCASTER` | `LazyLock<ContextBroadcaster>` | Aggregated context events |
| `ANALYTICS_BROADCASTER` | `LazyLock<AnalyticsBroadcaster>` | Analytics event tracking |

## Tests

Tests are located in `crates/tests/unit/infra/events/` following the project convention of separating tests from source files.

## Dependencies

| Crate | Purpose |
|-------|---------|
| `systemprompt-models` | Event types (`AgUiEvent`, `A2AEvent`, `ContextEvent`, `SystemEvent`, `ToSse` trait) |
| `systemprompt-identifiers` | `UserId` type |
| `tokio` | Async runtime, channels, synchronization |
| `axum` | SSE `Event` and `KeepAlive` types |
| `async-trait` | Async trait support |
| `tracing` | Structured logging |

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial license. Each version converts to Apache 2.0 four years after publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).

---

<div align="center">

**[systemprompt.io](https://systemprompt.io)** · **[Documentation](https://systemprompt.io/documentation/)** · **[Guides](https://systemprompt.io/guides)** · **[Live Demo](https://systemprompt.io/features/demo)** · **[Template](https://github.com/systempromptio/systemprompt-template)** · **[crates.io](https://crates.io/crates/systemprompt-events)** · **[docs.rs](https://docs.rs/systemprompt-events)** · **[Discord](https://discord.gg/wkAbSuPWpr)**

<sub>Infra layer · Own how your organization uses AI.</sub>

</div>
