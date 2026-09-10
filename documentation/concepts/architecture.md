# Architecture

How systemprompt-core is organised into layers, how a request flows through them, and how the runtime container is assembled at startup.

The root workspace contains 34 members, including the `systemprompt` facade. Production crates are arranged into five layers. `Cargo.toml` defines workspace membership; `scripts/lint-layers.sh` checks dependency boundaries. The bridge and tests use separate workspaces.

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
│  APP      runtime, scheduler, generator        │  cross-domain orchestration
└───────────────────┬───────────────────────────┘
                    │
┌───────────────────▼───────────────────────────┐
│  DOMAIN   users oauth files analytics content  │  bounded contexts (SQL + repos
│           ai mcp agent templates               │  + services), no cross-domain deps
└───────────────────┬───────────────────────────┘
                    │
┌───────────────────▼───────────────────────────┐
│  INFRA    database events security config      │  stateless cross-cutting utilities
│           logging loader cloud storage                 │
└───────────────────┬───────────────────────────┘
                    │
┌───────────────────▼───────────────────────────┐
│  SHARED   models traits identifiers extension  │  types and shared contracts
│           provider-contracts client            │
│           template-provider                    │
└─────────────────────────────────────────────────┘
```

| Layer | What it contains | What it may depend on |
|-------|------------------|-----------------------|
| Shared | Shared types, traits, identifiers, clients and integration primitives. | Other shared crates only |
| Infra | Stateless cross-cutting utilities (connection pooling, JWT validation, config loading, tracing, the event bus). I/O is allowed; persistent domain state is not. | Shared |
| Domain | Bounded contexts. Each owns its database tables, repositories (`src/repository/`), and services (`src/services/`). | Shared, Infra |
| App | Orchestration of multiple domains for workflows. No business logic of its own. | Shared, Infra, Domain |
| Entry | Binaries and the HTTP surface. Pure wiring. | All layers |
| Facade | `systemprompt`: feature-gated re-exports for external consumers on crates.io. | All layers |

### Why domains do not depend on each other

Domain crates do not depend on other domain crates. A capability one domain needs from another is reached in one of two ways:

- A trait defined in `shared/traits` (or `shared/provider-contracts`), implemented by the providing domain and consumed as `Arc<dyn Trait>` by the dependent one.
- An event published through `infra/events` and observed by a subscriber.

Cross-domain orchestration that would otherwise create a domain-to-domain edge is lifted up a layer. The runtime, scheduler and generator compose domain services through their dependencies and extension contracts. The wiring of all domains together happens in `entry/api`.

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

`AppContext` (`crates/app/runtime/src/context/`) is the application-wide runtime container. It holds the config, database pool, extension registry, analytics service, route classifier, MCP registry, the authorization hook, and other shared handles. Every field is an `Arc` (or an `Arc`-internal handle such as `DbPool`), so cloning the context is a reference-count bump, not a deep copy. The HTTP server, the scheduler, and CLI commands all clone it freely into handlers, jobs, and spawned tasks.

Some handles are optional — `geoip_reader`, `content_config`, `fingerprint_repo`, and `user_service` are `None` when the corresponding resource is absent or failed to initialise, and callers degrade rather than assume presence.

The context is assembled by `AppContextBuilder::build` (`crates/app/runtime/src/builder/`), which owns a fixed bootstrap order. The configuration half of that order runs earlier, in `infra/config`:

```
ProfileBootstrap  →  SecretsBootstrap  →  CredentialsBootstrap  →  Config  →  AppContext
```

1. **ProfileBootstrap** loads the active `profile.yaml`. Profiles are the single source of truth for configuration; there are no environment-variable fallbacks for profile values (`${VAR}` interpolation inside profile values is supported). See [the configuration guide](../guides/configure.md).
2. **SecretsBootstrap** loads the secrets envelope. The envelope is customer-owned; the binary never holds the master key.
3. **CredentialsBootstrap** loads cloud credentials where present.
4. **Config** builds the validated configuration object and confirms required paths exist.
5. **AppContext** is assembled. Within `AppContextBuilder::build` the steps run in order: profile → app paths → files config → database pool → authorization hook (built after the pool so its audit sink can write `governance_decisions`) → logging → extension registry (discovered via `inventory` and validated, with schema installation optionally applied) → ancillary services (analytics, fingerprint repo, user service, system admin, MCP registry, marketplace filter).

A failure at any step propagates as a `RuntimeError` and aborts startup. Configuration and schema validation are blocking by design — there is no `--force` bypass.

### Shutdown

The API handles Ctrl-C and, on Unix, SIGTERM. It signals shutdown readiness and starts graceful connection draining with a 10-second deadline. Teardown then stops the scheduler and terminates registered child processes; child termination has a 5-second grace period and teardown has a separate 10-second forced-exit deadline. A second shutdown signal exits immediately. Long-lived connections can be interrupted when the drain deadline expires. See `crates/entry/api/src/services/server/shutdown.rs`.

## See also

- [extensions.md](extensions.md) — how compile-time extensions plug capabilities into this graph without creating cross-crate edges.
- [authentication.md](authentication.md) — the authentication and authorization controls in the request path.
- [The deployment reference architecture](../guides/deploy-production.md) — how the layers map onto a deployed topology.
