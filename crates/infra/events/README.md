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
[![codecov](https://img.shields.io/codecov/c/github/systempromptio/systemprompt-core/main?style=flat-square&logo=codecov)](https://codecov.io/gh/systempromptio/systemprompt-core)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

Every A2A, AG-UI, analytics, and context event fans out through one bus, and a durable Postgres outbox carries it across replicas so nothing is lost to a restart. Any component holding a `UserId` can publish without touching the wire format.

**Layer**: Infra. Infrastructure primitives consumed by the domain and application crates. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## What it does

The in-process event bus fans typed events out to per-user SSE connections, managing connection lifecycles and cleaning up disconnected clients. It is not SSE-only: a durable Postgres outbox (`event_outbox` table, via LISTEN/NOTIFY) relays events across replicas so a multi-instance deployment stays consistent. The crate is shared between the HTTP API entry crate and the runtime layer.

## Modules

| Module | Purpose |
|--------|---------|
| `services` | The `GenericBroadcaster` implementation and per-event aliases, the static `EventRouter`, `ConnectionGuard`, keep-alive utilities, the `PostgresEventBridge` (LISTEN/NOTIFY relay), and the outbox repository. |
| `sse` | The `ToSse` trait and `serde`-driven impls converting `systemprompt-models` event types into `axum` SSE records. |
| `extension` | `EventsExtension` declares the `event_outbox` schema through the workspace extension framework. |
| `error` | `EventError` / `EventResult`. |

Schema DDL lives in `schema/` (`event_outbox.sql` plus the `migrations/` directory).

### Event flow

```
                    ┌─────────────────┐
                    │   EventRouter   │
                    └────────┬────────┘
                             │
        ┌────────────────────┼────────────────────┐
        │                    │                    │
        ▼                    ▼                    ▼
┌───────────────┐    ┌───────────────┐    ┌───────────────────┐
│AGUI_BROADCASTER│    │A2A_BROADCASTER│    │CONTEXT_BROADCASTER│
└───────┬───────┘    └───────┬───────┘    └─────────┬─────────┘
        │                    │                      │
        ▼                    ▼                      ▼
   SSE Clients          SSE Clients            SSE Clients
```

AG-UI and A2A events route to both their primary broadcaster and the context broadcaster for aggregation. Across replicas, `PostgresEventBridge` relays outbox rows so an instance publishes locally and every other instance sees it.

## Usage

```toml
[dependencies]
systemprompt-events = "0.21"
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
| `EventSender` | `tokio::sync::mpsc::Sender<Result<Event, Infallible>>` |
| `EventError` / `EventResult<T>` | `thiserror`-derived error and result alias |
| `GenericBroadcaster<E>` | Generic broadcaster for any `ToSse + Clone + Send + Sync` event |
| `AgUiBroadcaster` | `GenericBroadcaster<AgUiEvent>` |
| `A2ABroadcaster` | `GenericBroadcaster<A2AEvent>` |
| `ContextBroadcaster` | `GenericBroadcaster<ContextEvent>` |
| `AnalyticsBroadcaster` | `GenericBroadcaster<AnalyticsEvent>` |
| `ConnectionGuard<E>` | RAII guard for automatic unregistration |
| `EventRouter` | Routes events to appropriate broadcasters |
| `PostgresEventBridge` | LISTEN/NOTIFY relay draining the `event_outbox` table across replicas |
| `OutboxChannel` / `OUTBOX_CHANNEL` | Outbox notification channel type and name |
| `EventsExtension` | Extension registering the `event_outbox` schema |

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
| `systemprompt-models` | Event types (`AgUiEvent`, `A2AEvent`, `ContextEvent`, `SystemEvent`, `AnalyticsEvent`) |
| `systemprompt-identifiers` | `UserId` / `ConnectionId` types (`sqlx` feature) |
| `systemprompt-extension` | Schema registration for the `event_outbox` table |
| `tokio` | Async runtime, channels, synchronization |
| `axum` | SSE `Event` and `KeepAlive` types |
| `sqlx` | Durable outbox relay over Postgres LISTEN/NOTIFY |
| `chrono` | Outbox row timestamps |
| `inventory` | Compile-time extension registration |
| `serde` / `serde_json` | Event serialization |
| `tracing` | Structured logging |

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial license. Each version converts to Apache 2.0 four years after publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).

---

<div align="center">

**[systemprompt.io](https://systemprompt.io)** · **[Documentation](https://systemprompt.io/documentation/)** · **[Guides](https://systemprompt.io/guides)** · **[Live Demo](https://systemprompt.io/features/demo)** · **[Template](https://github.com/systempromptio/systemprompt-template)** · **[crates.io](https://crates.io/crates/systemprompt-events)** · **[docs.rs](https://docs.rs/systemprompt-events)** · **[Discord](https://discord.gg/wkAbSuPWpr)**

<sub>Infra layer · Own how your organization uses AI.</sub>

</div>
