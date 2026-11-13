# The extension model

How systemprompt-core lets downstream projects add capabilities — schemas, routes, jobs, providers — at compile time, through link-time discovery rather than runtime plugin loading.

systemprompt-core is a library you compile into your binary, not a framework you drop plugins into. Extensions are Rust types that implement the `Extension` trait and register themselves at compile time. Extensions are linked into the binary and discovered at startup through registered factories. The registry stores trait objects; adding compiled code requires rebuilding.

## The `Extension` trait

A single trait, `Extension`, aggregates every capability a crate can contribute. Every capability method has a default that returns "nothing", so an extension implements only `metadata()` and overrides the handful of methods for the capabilities it actually provides.

```rust
use systemprompt_extension::prelude::*;

#[derive(Default)]
struct MyExtension;

impl Extension for MyExtension {
    fn metadata(&self) -> ExtensionMetadata { /* id, name, version */ }

    fn schemas(&self) -> Vec<SchemaDefinition> { /* tables this extension owns */ }
    fn router(&self, ctx: &dyn ExtensionContext) -> Option<ExtensionRouter> { /* HTTP routes */ }
    fn jobs(&self) -> Vec<Arc<dyn Job>> { /* background jobs */ }
    // ...everything else defaults to empty
}

register_extension!(MyExtension);
```

The capability surface includes schemas, migrations, seeds and retirements, an HTTP router, background jobs and runner services, config planes, configuration schema and validation, readiness checks and metric descriptors, tool providers, a family of content/template/rendering providers, required storage paths and assets, RBAC roles, declared dependencies on other extensions, and typed shared state. AI model providers are configured through the profile and the gateway, not contributed by extensions. An extension opts into exactly what it needs by overriding the corresponding method.

### Typed extension state

The platform has one type-keyed map, `Dependencies`, used for extension state, job contexts, config-plane contexts and prerender contexts alike. At boot the host seeds it with the kernel's own handles (database pool, paths, files configuration, notification configuration, the documents service when the document store is enabled, and on the API also the OAuth and AI repositories and, with the brain feature, the brain configuration and notification sink), then `Extension::provide(ctx, state)` runs once for every enabled extension, in dependency order, and contributes values keyed by type. The host owns the finished state; it is immutable after boot and reached by every factory through `ExtensionContext::extension_state()`, and job and runner contexts start from it. This is how an extension builds a repository, service or cache once and shares it between its router, readiness checks, gateway guards, authz hook and other seams without a global.

The authz hook is built before the extension state exists, because the hook is an input to the context extensions provide from. Its factories receive a `StateHandle` in `AuthzHookContext::state`; the host binds the handle to the finished state before anything serves, and a read before binding fails closed.

Boot validates the graph before anything serves: an unknown dependency, a dependency cycle, a second provider of a type the kernel or another extension already provided, or a type declared in `Extension::requires()` that neither the kernel nor any enabled extension provides is a typed startup error, never a failure on a request. An extension listed in the profile's `extensions.disabled` neither provides state nor has its requirements checked; an enabled extension that requires a type only a disabled one provides fails boot, and one that depends on a disabled extension is refused.

### The extension context

Every factory receives an `&dyn ExtensionContext`. It hands an extension the profile, the resolved configuration, the services view and root, the database handle, file storage, secrets, background tasks, the instance id, a token authority, subject attributes, provider tokens, the cross-replica coordination seams (instances, leases, advisory locks) and the extension state. There are no separate typed sub-traits to implement. The host context also implements four narrow capability traits — `HasAnalytics`, `HasFingerprint`, `HasUserService` and `HasRouteClassifier` — for host code that needs those services directly. Extensions that govern gateway requests implement `GatewayRequestGuard` and register it with `register_gateway_guard!`.

## Registration and link-time discovery

`register_extension!` does not call anything at module-load time. It emits an `inventory::submit!` of a small factory that constructs the extension as `Arc<dyn Extension>`:

```rust
inventory::submit! {
    ExtensionRegistration { factory: || Arc::new(MyExtension::default()) as Arc<dyn Extension> }
}
```

The `inventory` crate collects every such submission across all linked crates into a single static set. At startup, `ExtensionRegistry::discover()` walks that set, instantiates each extension, and validates the result; the registry is then stored on the `AppContext`. This is link-time discovery: an extension is found because its crate is in the binary, not because a directory was scanned or a shared object was loaded.

```
crate A  register_extension!(A)  ─┐
crate B  register_extension!(B)  ─┼─►  inventory static set
crate C  register_extension!(C)  ─┘          │
                                             ▼
                              ExtensionRegistry::discover()
                                  ├─ schema_extensions → install schemas and migrations
                                  ├─ build_state       → kernel handles + provide, requires checked
                                  ├─ api_routers       → mount routes
                                  ├─ job_extensions    → scheduler
                                  └─ config_extensions → startup validation
```

### The product-binary requirement

Because `inventory` statics are only linked when their crate is part of the final binary, the product — not core's CLI — must own the binary. A product crate re-exports core plus its extensions and the binary references the product crate so its statics link. This is the trade-off of compile-time registration: the binary is the unit of composition, and adding an extension means rebuilding, not dropping a file into a directory.

This is also what keeps the layered architecture strict (see [architecture.md](architecture.md)). Because capabilities are discovered through `inventory` at link time, a domain crate never needs a compile-time dependency on another domain crate to reach its capability — which is why the dependency graph has zero cross-domain edges.

## Schema and migration embedding

An extension owns its database schema, and the schema travels inside the crate so it works when the crate is published to crates.io.

- **Schema DDL** lives in `{crate}/schema/*.sql` and is embedded with `include_str!` inside the extension's `schemas()` implementation. A missing file is a compile error, so the SQL cannot drift away from the code that references it.
- **Migrations** live in `{crate}/schema/migrations/NNN_<name>.sql`. A one-line `build.rs` calls `systemprompt_extension::build::emit_migrations()`, which discovers those files at build time and generates the body of `Extension::migrations()`; the extension consumes it through the `extension_migrations!()` macro. Version and name are derived from the filename, so adding a migration is adding one file — no Rust edit — and `cargo:rerun-if-changed` retriggers the build when a file appears.

> The extension framework's `build.rs` is the one sanctioned exception to the shared layer's no-I/O rule: it performs file I/O only at compile time and is never reached by any runtime code path.

The registry orders extensions once — every dependency before its dependents, ties broken by priority and then id — and that order drives schema installation, so a dependency's tables exist before its dependents' migrations run.

## See also

- [authoring extensions](../guides/authoring-extensions.md) — the step-by-step guide to building one.
- [architecture.md](architecture.md) — how extensions keep the dependency graph acyclic and cross-domain-free.
