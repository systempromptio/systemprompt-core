# Authoring an extension

How to build a systemprompt extension: declare metadata, embed schema DDL and migrations, mount API routes, and register the extension so the runtime discovers it at compile time.

An extension is a Rust type that implements the `Extension` trait and is registered with the `register_extension!` macro. Registrations are collected at compile time through the [`inventory`](https://docs.rs/inventory) crate — there is no runtime plugin loading, no dynamic library, and no manifest file to ship. An extension is code that links into the binary. For the design rationale, see [concepts/extensions.md](../concepts/extensions.md).

## Prerequisites

- A Rust toolchain matching the workspace edition (Edition 2024; minimum Rust per `rust-toolchain.toml`).
- A crate that depends on the `systemprompt` facade with the `core` feature, or directly on `systemprompt-extension`.
- The extension crate must be linked into the binary you run. Compile-time registration only takes effect for crates that are actually compiled and linked; an extension crate that nothing depends on contributes nothing.

```toml
# Cargo.toml
[dependencies]
systemprompt = { version = "0.11", features = ["core"] }
```

The trait, the macro, and the value types are re-exported through the facade prelude:

```rust
use systemprompt::extension::prelude::*;
```

This brings `Extension`, `ExtensionMetadata`, `SchemaDefinition`, `ExtensionRouter`, `Migration`, `register_extension!`, `extension_migrations!`, the typed sub-traits, and the `ExtensionContext` trait into scope (`crates/shared/extension/src/lib.rs:105`).

## Step 1 — Implement the `Extension` trait

`Extension` has one required method, `metadata`. Every other method has a default that returns an empty contribution, so you implement only the surfaces your extension provides (`crates/shared/extension/src/traits/extension.rs:18`).

`ExtensionMetadata` has three fields, all `&'static str` (`crates/shared/extension/src/metadata.rs:5`):

| Field | Meaning |
|-------|---------|
| `id` | Stable identifier, used for dependency references and registry lookup. |
| `name` | Display name. |
| `version` | Version string; `env!("CARGO_PKG_VERSION")` is the conventional value. |

```rust
use systemprompt::extension::prelude::*;

#[derive(Default)]
struct DemoExtension;

impl Extension for DemoExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "demo-extension",
            name: "Demo Extension",
            version: env!("CARGO_PKG_VERSION"),
        }
    }
}
```

The type must be `Send + Sync + 'static`. The `register_extension!($type)` form constructs the extension with `Default::default()`, so deriving `Default` is required when you register by type (`crates/shared/extension/src/traits/register.rs:3`).

## Step 2 — Register the extension

`register_extension!` submits an `inventory` registration. The runtime iterates every registration at startup, builds each extension, validates declared dependencies, and merges the schemas, routes, jobs, and providers into the host.

```rust
register_extension!(DemoExtension);
```

Two forms are accepted (`crates/shared/extension/src/traits/register.rs`):

- `register_extension!(DemoExtension)` — constructs via `Default`.
- `register_extension!(DemoExtension::with_config(cfg))` — registers a value you build.

The complete minimal extension is the facade example `systemprompt/examples/extension.rs`, runnable with:

```bash
cargo run -p systemprompt --example extension --features core
```

## Step 3 — Declare schema tables

Return a `SchemaDefinition` for each table the extension owns. Embed the DDL from a `.sql` file with `include_str!` rather than inlining a SQL string literal; the convention is `{crate}/schema/<table>.sql` (`crates/domain/users/src/extension.rs:19`).

```rust
fn schemas(&self) -> Vec<SchemaDefinition> {
    vec![
        SchemaDefinition::new("demo_items", include_str!("../schema/demo_items.sql"))
            .with_required_columns(vec!["id".into(), "created_at".into()]),
    ]
}
```

`SchemaDefinition::new(table, sql)` takes the table name and the DDL. `with_required_columns` records columns the runtime validates against the live schema. `with_schema(name)` targets a non-`public` Postgres schema; the default is `public` (`crates/shared/extension/src/metadata.rs:21`).

The DDL file (`schema/demo_items.sql`) holds the `CREATE TABLE`:

```sql
CREATE TABLE IF NOT EXISTS demo_items (
    id          TEXT PRIMARY KEY,
    label       TEXT NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

The tables an extension owns are derived from these `CREATE TABLE` statements. Do not list owned tables in `cross_extension_tables` — that method is only for tables another extension creates that this one is permitted to `ALTER` (`crates/shared/extension/src/traits/extension.rs:117`).

## Step 4 — Add migrations discovered by `build.rs`

Schema changes after the initial `CREATE TABLE` are migrations. Migration SQL lives in `{crate}/schema/migrations/NNN_<name>.sql`, where `NNN` is a numeric version prefix. The version and name come from the filename, so they cannot drift from the SQL they label (`crates/shared/extension/src/build.rs:40`).

1. Add a build script that emits the migration list:

   ```rust
   // build.rs
   fn main() {
       systemprompt_extension::build::emit_migrations();
   }
   ```

   Add the same dependency to `[build-dependencies]`:

   ```toml
   [build-dependencies]
   systemprompt-extension = "0.11"
   ```

2. Add migration files. For example `schema/migrations/001_add_demo_items_label_index.sql`:

   ```sql
   CREATE INDEX idx_demo_items_label ON demo_items (label);
   ```

3. Return the generated list from `migrations` with the `extension_migrations!` macro, which `include!`s the file `emit_migrations` wrote to `OUT_DIR` (`crates/shared/extension/src/migration.rs:16`):

   ```rust
   fn migrations(&self) -> Vec<Migration> {
       extension_migrations!()
   }
   ```

Filename conventions enforced by the build script (`crates/shared/extension/src/build.rs:14`):

| File | Effect |
|------|--------|
| `NNN_<name>.sql` | An up migration; `NNN` is the version, the remainder is the name. |
| `NNN_<name>.down.sql` | The optional paired down migration. |
| First non-blank line `-- @no-transaction` | Emitted with `Migration::new_no_transaction`, for statements Postgres rejects inside a transaction (for example `CREATE INDEX CONCURRENTLY`). A `-- @no-transaction` migration must not declare a `.down.sql`. |

The build fails if a file is misnamed or two files share a version. Adding a file retriggers the build through the emitted `cargo:rerun-if-changed` directive.

## Step 5 — Mount an API router

To serve HTTP routes, return an `ExtensionRouter` from `router`. The method receives an `&dyn ExtensionContext`, which exposes the config provider, the database handle, and lookup of other extensions (`crates/shared/extension/src/context.rs:6`).

```rust
fn router(&self, _ctx: &dyn ExtensionContext) -> Option<ExtensionRouter> {
    let router = axum::Router::new()
        .route("/items", axum::routing::get(|| async { "ok" }));
    Some(ExtensionRouter::new(router, "/api/v1/demo"))
}
```

`ExtensionRouter::new(router, base_path)` mounts under `base_path` and requires authentication. `ExtensionRouter::public(router, base_path)` mounts without an auth requirement (`crates/shared/extension/src/router.rs:42`). Choose `public` only for routes that must be reachable unauthenticated.

## Step 6 — Contribute jobs and providers (optional)

Two further `Extension` methods extend the runtime:

- `jobs` returns `Vec<Arc<dyn Job>>` — scheduled work registered with the scheduler (`crates/shared/extension/src/traits/extension.rs:37`).
- `llm_providers` and `tool_providers` return provider implementations the AI layer can use (`crates/shared/extension/src/traits/extension.rs:53`).

Both default to empty; implement them only when the extension supplies that surface.

## Typed extension sub-traits

Beyond the single `Extension` trait, the framework offers narrower typed contracts in `crates/shared/extension/src/typed/`. Each is built on the `ExtensionMeta` supertrait (`crates/shared/extension/src/types.rs:19`) and constrains an extension to one concern, which the dependency typestate can check at compile time:

| Trait | Required method(s) | Source |
|-------|--------------------|--------|
| `SchemaExtensionTyped` | `schemas() -> Vec<SchemaDefinitionTyped>` | `typed/schema.rs:32` |
| `ApiExtensionTyped` | `base_path() -> &'static str`; `requires_auth()` defaults to `true` | `typed/api.rs:8` |
| `JobExtensionTyped` | `jobs() -> Vec<Arc<dyn Job>>` | `typed/job.rs:10` |
| `ProviderExtensionTyped` | `llm_providers()` / `tool_providers()`, both default empty | `typed/provider.rs:10` |
| `ConfigExtensionTyped` | `config_prefix() -> &'static str`; `validate_config` / `config_schema` default | `typed/config.rs:9` |

`ApiExtensionTyped` pairs with `ApiExtensionTypedDyn`, which adds `build_router() -> Router` and keeps the router-building surface object-safe (`crates/shared/extension/src/typed/api.rs:16`).

## How discovery works

There is no runtime loader and no plugin file format. `register_extension!` expands to `inventory::submit!`, which places the registration in a link-time collection (`crates/shared/extension/src/traits/register.rs:4`). At startup the runtime iterates `inventory::iter` over those registrations, instantiates each extension, validates that every declared dependency is also registered, and orders the result by `priority` (default `100`). Because discovery is compile-time, an extension only participates if its crate is compiled into the running binary.

## Verify

```bash
cargo build -p <your-extension-crate>
cargo run -p systemprompt --example extension --features core
```

A clean build of a crate that links your extension is sufficient to confirm registration; there is no separate load step to check.

## Related pages

- [concepts/extensions.md](../concepts/extensions.md) — why the framework is compile-time and how the pieces fit together.
- [configure-providers.md](configure-providers.md) — configuring AI providers through the gateway.
