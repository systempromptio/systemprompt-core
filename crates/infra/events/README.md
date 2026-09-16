# systemprompt-events

Transactional reporting can use `services::durable::DurableOutbox`. Its `append`
method takes the source SQL transaction, actor, an existing typed SSE event and
a `ReportingFact<T>` with a consumer, kind and version. Use the primary database
for both the outbox pool and the source transaction. Committing publishes the
notification; rolling back removes both the event and notification. Run a bridge
on the emitting instance for local delivery of this opt-in path. Existing
`EventRouter::route_*` methods keep their immediate local delivery behavior.
Use one publishing path per event: do not call `route_*` again after `append`.

A consumer polls `claim`, validates the fact kind/version, applies projection
writes using `Delivery::connection`, and calls `acknowledge` to commit both writes
and processing state. Rollback or dropping the delivery leaves it pending.
Claims use row locks with `SKIP LOCKED`; they do not promise aggregate ordering.
Consumers must reject stale entity revisions and make external side effects
idempotent. Each row supports one durable consumer. Pending facts survive relay
cleanup; acknowledged rows retain the ordinary age-based retention policy.

Apply migration 004 and upgrade all relay instances before enabling durable
producers. Older binaries do not protect pending rows from pruning. SSE remains
a live stream: a disconnected listener can miss notifications; durable consumers
recover by polling. This API does not install a projector or change analytics
report queries. See the [analytics migration plan](../../../documentation/concepts/analytics-migration.md).

[![Crates.io](https://img.shields.io/crates/v/systemprompt-events.svg?style=flat-square)](https://crates.io/crates/systemprompt-events)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-events?style=flat-square)](https://docs.rs/systemprompt-events)
[![codecov](https://img.shields.io/codecov/c/github/systempromptio/systemprompt-core/main?style=flat-square&logo=codecov)](https://codecov.io/gh/systempromptio/systemprompt-core)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

Routes typed events through local broadcasters and a PostgreSQL outbox for cross-replica delivery. Connected-client delivery has separate buffering and replay semantics.

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
systemprompt-events = "0.53"
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
