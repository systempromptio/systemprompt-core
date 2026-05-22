# The extension model

How systemprompt-core lets downstream projects add capabilities — schemas, routes, jobs, providers — at compile time, through link-time discovery rather than runtime plugin loading.

systemprompt-core is a library you compile into your binary, not a framework you drop plugins into. Extensions are Rust types that implement the `Extension` trait and register themselves at compile time. There is no runtime plugin loading, no reflection, and no dynamic dispatch at the governance boundary — an extension's capabilities are linked into the binary and discovered when the process starts.

## The `Extension` trait

A single trait, `Extension` (`crates/shared/extension/src/traits/extension.rs:18`), aggregates every capability a crate can contribute. Every capability method has a default that returns "nothing", so an extension implements only `metadata()` and overrides the handful of methods for the capabilities it actually provides.

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

The capability surface includes schemas and migrations, an HTTP router, background jobs, configuration schema and validation, LLM and tool providers, a family of content/template/rendering providers, required storage paths, RBAC roles, and declared dependencies on other extensions. An extension opts into exactly what it needs by overriding the corresponding method.

### Typed variants

Beyond the base trait, typed extension traits in `crates/shared/extension/src/typed/` express specific capabilities with compile-time constants and stronger typing:

| Trait | Purpose |
|-------|---------|
| `SchemaExtensionTyped` | Database tables with migration weights |
| `ApiExtensionTyped` | HTTP route handlers with a base path and auth requirements |
| `JobExtensionTyped` | Background job definitions |
| `ProviderExtensionTyped` | Custom LLM / tool provider implementations |
| `ConfigExtensionTyped` | Configuration validation at startup |

## Registration and link-time discovery

`register_extension!` (`crates/shared/extension/src/traits/register.rs`) does not call anything at module-load time. It emits an `inventory::submit!` of a small factory that constructs the extension as `Arc<dyn Extension>`:

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
                                  ├─ schema_extensions  → install schemas (migration order)
                                  ├─ api_extensions     → mount routes
                                  ├─ job_extensions     → scheduler
                                  ├─ provider_extensions→ LLM / tool providers
                                  └─ config_extensions  → startup validation
```

### The product-binary requirement

Because `inventory` statics are only linked when their crate is part of the final binary, the product — not core's CLI — must own the binary. A product crate re-exports core plus its extensions and the binary references the product crate so its statics link. This is the trade-off of compile-time registration: the binary is the unit of composition, and adding an extension means rebuilding, not dropping a file into a directory.

This is also what keeps the layered architecture strict (see [architecture.md](architecture.md)). Because capabilities are discovered through `inventory` at link time, a domain crate never needs a compile-time dependency on another domain crate to reach its capability — which is why the dependency graph has zero cross-domain edges.

## Schema and migration embedding

An extension owns its database schema, and the schema travels inside the crate so it works when the crate is published to crates.io.

- **Schema DDL** lives in `{crate}/schema/*.sql` and is embedded with `include_str!` inside the extension's `schemas()` implementation. A missing file is a compile error, so the SQL cannot drift away from the code that references it.
- **Migrations** live in `{crate}/schema/migrations/NNN_<name>.sql`. A one-line `build.rs` calls `systemprompt_extension::build::emit_migrations()`, which discovers those files at build time and generates the body of `Extension::migrations()`; the extension consumes it through the `extension_migrations!()` macro. Version and name are derived from the filename, so adding a migration is adding one file — no Rust edit — and `cargo:rerun-if-changed` retriggers the build when a file appears.

> The extension framework's `build.rs` is the one sanctioned exception to the shared layer's no-I/O rule: it performs file I/O only at compile time and is never reached by any runtime code path.

Migration weights order schema installation so dependencies exist before dependents — core tables (weights 1–35) install before extension tables (weights 100+).

## See also

- [authoring extensions](../guides/authoring-extensions.md) — the step-by-step guide to building one.
- [architecture.md](architecture.md) — how extensions keep the dependency graph acyclic and cross-domain-free.
