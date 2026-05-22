# Architecture

How systemprompt-core is organised into layers, how a request flows through them, and how the runtime container is assembled at startup.

systemprompt-core is a Rust workspace of 31 members — 30 published crates plus the `systemprompt` facade — arranged into five layers. The layering is not a convention enforced only by review: it is enforced by the dependency graph itself. Across all 30 non-test crates there are no upward edges, no edges between domain crates, and no dependency cycles.

## The five layers

Dependencies flow in one direction only — downward. A crate may depend on crates in its own layer or any layer below it, never above.

```
┌─────────────────────────────────────────────┐
│  FACADE   systemprompt                        │  re-exports, feature-gated
└───────────────────┬───────────────────────────┘
                    │
┌───────────────────▼───────────────────────────┐
│  ENTRY    api, cli                             │  wiring, no business logic
└───────────────────┬───────────────────────────┘
                    │
┌───────────────────▼───────────────────────────┐
│  APP      runtime, scheduler, generator, sync  │  cross-domain orchestration
└───────────────────┬───────────────────────────┘
                    │
┌───────────────────▼───────────────────────────┐
│  DOMAIN   users oauth files analytics content  │  bounded contexts (SQL + repos
│           ai mcp agent templates               │  + services), no cross-domain deps
└───────────────────┬───────────────────────────┘
                    │
┌───────────────────▼───────────────────────────┐
│  INFRA    database events security config      │  stateless cross-cutting utilities
│           logging loader cloud                 │
└───────────────────┬───────────────────────────┘
                    │
┌───────────────────▼───────────────────────────┐
│  SHARED   models traits identifiers extension  │  pure types, zero I/O
│           provider-contracts client            │
│           template-provider                    │
└─────────────────────────────────────────────────┘
```

| Layer | What it contains | What it may depend on |
|-------|------------------|-----------------------|
| Shared | Type definitions, trait definitions, constants, pure functions. No I/O, no SQL, no global mutable state. | Other shared crates only |
| Infra | Stateless cross-cutting utilities (connection pooling, JWT validation, config loading, tracing, the event bus). I/O is allowed; persistent domain state is not. | Shared |
| Domain | Bounded contexts. Each owns its database tables, repositories (`src/repository/`), and services (`src/services/`). | Shared, Infra |
| App | Orchestration of multiple domains for workflows. No business logic of its own. | Shared, Infra, Domain |
| Entry | Binaries and the HTTP surface. Pure wiring. | All layers |
| Facade | `systemprompt`: feature-gated re-exports for external consumers on crates.io. | All layers |

### Why domains do not depend on each other

The strongest property of the graph is that no domain crate depends on another domain crate. A capability one domain needs from another is reached in one of two ways:

- A trait defined in `shared/traits` (or `shared/provider-contracts`), implemented by the providing domain and consumed as `Arc<dyn Trait>` by the dependent one.
- An event published through `infra/events` and observed by a subscriber.

Cross-domain orchestration that would otherwise create a domain-to-domain edge is lifted up a layer. `runtime` depends on `mcp`, `users`, and `files`; `sync` depends on `agent` and `content`; `generator` depends on `content`, `files`, `templates`, and `sync`. The wiring of all domains together happens in `entry/api`.

This is why the extension framework matters to the layering (see [extensions.md](extensions.md)): capabilities are discovered at link time through the `inventory` crate rather than wired through compile-time dependency edges, so a domain never needs to name another domain to reach it.

## Request data flow

An inbound HTTP request is handled entirely in the entry layer's middleware and route stack, calling down into domain services through the shared `AppContext`. The ordering below is the path through `entry/api`; the authorization controls are described in [authentication.md](authentication.md).

```
HTTP request
   │
   ▼
client-IP resolution        (trusted-proxy gated; feeds rate-limit / IP-ban / bot controls)
   │
   ▼
rate limit · IP ban · bot checks
   │
   ▼
JWT extraction + validation  (RS256, kid, exp/nbf/iat, act-chain cap)
   │
   ▼
authorization hook           (fail-closed default-deny; webhook / disabled / unrestricted modes)
   │
   ▼
route handler                (calls a domain service via AppContext)
   │
   ▼
domain service ── repository ── database (compile-time-checked SQL)
   │
   ▼
response  (+ x-trace-id echoed; access logged to the logs table and to tracing)
```

Every layer in this path obtains the resources it needs from a single shared container, the `AppContext`.

## AppContext and the bootstrap order

`AppContext` (`crates/app/runtime/src/context.rs:41`) is the application-wide runtime container. It holds the config, database pool, extension registry, analytics service, route classifier, MCP registry, the authorization hook, and other shared handles. Every field is an `Arc` (or an `Arc`-internal handle such as `DbPool`), so cloning the context is a reference-count bump, not a deep copy. The HTTP server, the scheduler, and CLI commands all clone it freely into handlers, jobs, and spawned tasks.

Some handles are optional — `geoip_reader`, `content_config`, `fingerprint_repo`, and `user_service` are `None` when the corresponding resource is absent or failed to initialise, and callers degrade rather than assume presence.

The context is assembled by `AppContextBuilder::build` (`crates/app/runtime/src/builder.rs:97`), which owns a fixed bootstrap order. The configuration half of that order runs earlier, in `infra/config`:

```
ProfileBootstrap  →  SecretsBootstrap  →  CredentialsBootstrap  →  Config  →  AppContext
```

1. **ProfileBootstrap** loads the active `profile.yaml`. Profiles are the single source of truth for configuration; there are no environment-variable fallbacks for profile values (`${VAR}` interpolation inside profile values is supported). See [the configuration guide](../guides/configure.md).
2. **SecretsBootstrap** loads the secrets envelope. The envelope is customer-owned; the binary never holds the master key.
3. **CredentialsBootstrap** loads cloud credentials where present.
4. **Config** builds the validated configuration object and confirms required paths exist.
5. **AppContext** is assembled. Within `AppContextBuilder::build` the steps run in order: profile → app paths → files config → database pool → authorization hook (built after the pool so its audit sink can write `governance_decisions`) → logging → extension registry (discovered via `inventory` and validated, with schema installation optionally applied) → ancillary services (analytics, fingerprint repo, user service, system admin, MCP registry, marketplace filter).

A failure at any step propagates as a `RuntimeError` and aborts startup. Configuration and schema validation are blocking by design — there is no `--force` bypass.

### Known limitation

The main HTTP API server does not perform a graceful shutdown. `axum::serve` is invoked without `.with_graceful_shutdown`, and the in-process readiness-signalling path (`signal_shutdown()`) is not wired to a caller. On `SIGTERM`, in-flight requests and SSE connections are dropped rather than drained. The A2A agent server and the MCP/agent orchestrator daemons do handle their own shutdown signals. State this honestly when planning rolling deployments.

## See also

- [extensions.md](extensions.md) — how compile-time extensions plug capabilities into this graph without creating cross-crate edges.
- [authentication.md](authentication.md) — the authentication and authorization controls in the request path.
- [The deployment reference architecture](../guides/deploy-production.md) — how the layers map onto a deployed topology.
