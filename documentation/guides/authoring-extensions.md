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
systemprompt = { version = "0.81", features = ["core"] }
inventory = "0.3"
```

The trait, the macro, and the value types are re-exported through the facade prelude:

```rust
use systemprompt::extension::prelude::*;
```

This brings `Extension`, `ExtensionMetadata`, `SchemaDefinition`, `ExtensionRouter`, `Migration`, `register_extension!`, `extension_migrations!`, `register_gateway_guard!`, the `ExtensionContext` trait, the state types (`StateBuilder`, `StateRequirement`, `StateHandle`, `ExtensionStateError`, `Dependencies`) and the provider contracts into scope.

## Step 1 — Implement the `Extension` trait

`Extension` has one required method, `metadata`. Every other method has a default that returns an empty contribution, so you implement only the surfaces your extension provides.

`ExtensionMetadata` has three fields, all `&'static str`:

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

The type must be `Send + Sync + 'static`. The `register_extension!($type)` form constructs the extension with `Default::default()`, so implementing or deriving `Default` is required when you register by type.

## Step 2 — Register the extension

`register_extension!` submits an `inventory` registration. The runtime iterates every registration at startup, builds each extension, validates declared dependencies, and merges the schemas, routes, jobs, and providers into the host.

```rust
register_extension!(DemoExtension);
```

Two forms are accepted:

- `register_extension!(DemoExtension)` — constructs via `Default`.
- `register_extension!(DemoExtension::default())` — registers a factory expression. The expression runs in a noncapturing factory and cannot refer to local variables.

The complete minimal extension is the facade example `systemprompt/examples/extension.rs`, runnable with:

```bash
cargo run -p systemprompt --example extension --features core
```

## Step 3 — Declare schema tables

Return a `SchemaDefinition` for each table the extension owns. Embed the DDL from a `.sql` file with `include_str!` rather than inlining a SQL string literal; the convention is `{crate}/schema/<table>.sql`.

```rust
fn schemas(&self) -> Vec<SchemaDefinition> {
    vec![
        SchemaDefinition::new("demo_items", include_str!("../schema/demo_items.sql"))
            .with_required_columns(vec!["id".into(), "created_at".into()]),
    ]
}
```

`SchemaDefinition::new(table, sql)` takes the table name and the DDL. `with_required_columns` records columns the runtime validates against the live schema. `with_schema(name)` targets a non-`public` Postgres schema; the default is `public`.

The DDL file (`schema/demo_items.sql`) holds the `CREATE TABLE`:

```sql
CREATE TABLE IF NOT EXISTS demo_items (
    id          TEXT PRIMARY KEY,
    label       TEXT NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

The tables an extension owns are derived from these `CREATE TABLE` statements. Do not list owned tables in `cross_extension_tables` — that method is only for tables another extension creates that this one is permitted to `ALTER`.

## Step 4 — Add migrations discovered by `build.rs`

Schema changes after the initial `CREATE TABLE` are migrations. Migration SQL lives in `{crate}/schema/migrations/NNN_<name>.sql`, where `NNN` is a numeric version prefix. The version and name come from the filename, so they cannot drift from the SQL they label.

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
   systemprompt-extension = "0.81"
   ```

2. Add migration files. For example `schema/migrations/001_add_demo_items_label_index.sql`:

   ```sql
   CREATE INDEX idx_demo_items_label ON demo_items (label);
   ```

3. Return the generated list from `migrations` with the `extension_migrations!` macro, which `include!`s the file `emit_migrations` wrote to `OUT_DIR`:

   ```rust
   fn migrations(&self) -> Vec<Migration> {
       extension_migrations!()
   }
   ```

Filename conventions enforced by the build script:

| File | Effect |
|------|--------|
| `NNN_<name>.sql` | An up migration; `NNN` is the version, the remainder is the name. |
| `NNN_<name>.down.sql` | The optional paired down migration. |
| First non-blank line `-- @no-transaction` | Emitted with `Migration::new_no_transaction`, for statements Postgres rejects inside a transaction (for example `CREATE INDEX CONCURRENTLY`). A `-- @no-transaction` migration must not declare a `.down.sql`. |
| `NNN_<name>.tombstone` | Records the slot as spent. No SQL — the body is prose explaining why. |
| `NNN-MMM_<name>.tombstone` | The same, for an inclusive range of spent slots. |

The build fails if a file is misnamed or two files claim the same version. Adding a file retriggers the build through the emitted `cargo:rerun-if-changed` directive.

### Deleting a migration

**Never free a number by deleting its file.** A migration that has shipped is applied in every established database, which keeps a tracking row for that version forever. Delete the file and the number *looks* free, but refilling it means the runner sees the version as already applied, skips the new SQL entirely, and then refuses to boot on a checksum belonging to a migration nobody can find.

So deleting is two steps: remove the `.sql` and leave a `.tombstone` in its place, keeping the original name.

```
schema/migrations/
  007_add_index.sql          ->  007_add_index.tombstone
  008_next_change.sql
```

The runner never executes, records, or checksums a tombstoned slot, and `emit_migrations` treats the number as occupied — so reusing it fails the **build** rather than a deployment. `infra db migrate-status` lists any applied version that no file claims as `orphaned`; that is the list of tombstones still owed.

## Step 5 — Mount an API router

To serve HTTP routes, return an `ExtensionRouter` from `router`. The method receives an `&dyn ExtensionContext`, which exposes the profile and configuration, the database handle, file storage, secrets, background tasks, the coordination seams, lookup of other extensions and the extension state.

```rust
fn router(&self, _ctx: &dyn ExtensionContext) -> Option<ExtensionRouter> {
    let router = axum::Router::new()
        .route("/items", axum::routing::get(|| async { "ok" }));
    Some(ExtensionRouter::new(router, "/api/v1/demo"))
}
```

`ExtensionRouter::new(router, base_path)` mounts under `base_path` and requires authentication. `ExtensionRouter::public(router, base_path)` mounts without an auth requirement. Choose `public` only for routes that must be reachable unauthenticated.

## Step 6 — Provide shared state (optional)

An extension that builds a cache, a repository or a service once and reads it from several factories (its router, readiness checks, gateway guards, authz hook, notification or brain seams) provides it as typed state instead of holding it in a global.

```rust
use systemprompt_extension::prelude::*;

struct GrantCache { /* ... */ }

impl Extension for MyExtension {
    fn dependencies(&self) -> Vec<&'static str> {
        vec!["base"]
    }

    fn provide(
        &self,
        ctx: &dyn ExtensionContext,
        state: &mut StateBuilder<'_>,
    ) -> Result<(), ExtensionStateError> {
        let base = state.state().get_arc::<BaseService>()?;
        state.provide(GrantCache::new(ctx.database(), base))
    }

    fn requires(&self) -> Vec<StateRequirement> {
        vec![StateRequirement::of::<BaseService>()]
    }

    fn router(&self, ctx: &dyn ExtensionContext) -> Option<ExtensionRouter> {
        let cache = ctx.extension_state().get_arc::<GrantCache>().ok()?;
        /* build the router over `cache` */
    }
}
```

- The state is one `Dependencies` map. The host seeds it with the kernel's handles (database pool, paths, files configuration, notification configuration, the documents service when enabled, and on the API the OAuth and AI repositories), then `provide` runs once at boot, synchronously, for every enabled extension in registry order: every dependency before its dependents, ties broken by `(priority, id)`. During `provide` an extension reads the kernel's handles and what its dependencies already provided through `state.state()`; reading another extension's value while providing requires declaring that extension in `dependencies()`.
- Each type has exactly one provider. Providing a type the kernel or another extension already provided fails boot with an error naming both providers.
- `requires` declares types the extension cannot run without. After every `provide` has run, boot fails if any declared type is absent, naming the extension and the type, before any route is served. A requirement is met by any enabled provider, because requirements are checked once against the finished state that every later read sees.
- Extensions listed in the profile's `extensions.disabled` neither provide nor have their requirements checked. An enabled extension that requires a type only a disabled extension provides fails boot; one that depends on a disabled extension is refused.
- The finished state is immutable. Every factory reads it through `ctx.extension_state()`: `get::<T>()` borrows the value and `get_arc::<T>()` returns a shared `Arc<T>`; both return a `MissingDependency` naming the type when it is absent. Job and runner-service contexts start from the same state, so `ctx.get::<T>()` in a job finds provided values too. Values that change at run time hold their own synchronisation inside the provided type.
- An authz hook factory (`register_authz_hook!`) runs before the state exists. It receives `AuthzHookContext::state`, a `StateHandle` the host binds to the finished state before anything serves; the hook captures the handle and reads `state.get_arc::<T>()` when it evaluates. A read before binding fails with `ExtensionStateError::NotReady`, so the hook must deny rather than allow.
- An unknown dependency id or a dependency cycle (the error names the cycle) fails registry construction before any `provide` runs.
- Managed MCP servers run the same `provide` pass over their own context, so a seam factory finds the same types there.

## Step 7 — Contribute jobs and providers (optional)

Further `Extension` methods extend the runtime:

- `jobs` returns `Vec<Arc<dyn Job>>` — scheduled work registered with the scheduler.
- `runner_services` returns long-running services the server supervises.
- `config_planes` returns config planes reconciled against the services tree.
- `tool_providers` returns tool provider implementations the AI layer can call.
- `template_providers`, `component_renderers`, `template_data_extenders`, `page_data_providers`, `page_prerenderers`, `frontmatter_processors`, `content_data_providers`, `rss_feed_providers` and `sitemap_providers` contribute to content rendering and publishing.

All default to empty; implement them only when the extension supplies that surface. AI model providers are not contributed by extensions; they are configured in the profile and served through the gateway.

## The extension context

The framework exposes a single `Extension` trait. There are no typed sub-traits: what an extension needs from its host it reads from the `&dyn ExtensionContext` every factory receives (profile, configuration, services, database handle, file storage, secrets, background tasks, instance id, token authority, subject attributes, provider tokens, coordination seams and the extension state). The host context also implements `HasAnalytics`, `HasFingerprint`, `HasUserService` and `HasRouteClassifier` for host code that needs those services directly. Extensions that participate in gateway request governance implement `GatewayRequestGuard` and register it with `register_gateway_guard!`.

## Runtime seams

Core owns these contracts so an extension never reimplements them against core tables:

- **Governance:** build a `PolicyRequest` and call `GovernanceEngine::charge_and_evaluate`. The engine charges every armed rate window before the chain runs, so a caller cannot evaluate an uncharged call. A chain that arms `rate_limit` without a rate-window repository refuses to build.
- **Replica liveness:** `ctx.instances().is_alive(&instance_id)` is true while that replica holds its boot-time instance claim or heartbeats its `services` rows within 90 seconds. Use it instead of querying `services` with a staleness window of your own. `ctx.instance_id()` is this node's identity.
- **Leases:** `ctx.leases().claim(&LeaseKey::new("…"), ttl)` grants an expiring single-owner claim on a named resource, or `None` while another live replica holds it. Claim again before `ttl` elapses to keep it; a dead owner's lease is forfeit even before it expires. `release` and `holder` complete the set.
- **Advisory locks:** `ctx.advisory_locks().try_acquire(&AdvisoryLockKey::new("…"))` (or `acquire_within(key, wait)`) takes a cluster-wide lock on the session pool, never on a pool behind a transaction-mode pooler. Call `release().await` on the guard; dropping it closes the locking session, which also frees the lock. The key space is disjoint from the scheduler's job locks.
- **Quota usage:** read a quota window through `QuotaUsageRepository` (`ai_repositories().quota_usage`): `usage(window, subject_id)` and `window_usage(window, limit)` sum every shard row of the window. Never sum `ai_quota_buckets` directly; a window may be spread over several shards.
- **Job database budgets:** override `Job::db_budget` to return a `DbBudget` (statement timeout, optional lock timeout and `work_mem`), then open the job's transactions with `ctx.get::<BudgetedDb>()?.begin()`. The budget is applied with `SET LOCAL`, so a runaway statement is cancelled and rolled back without the limit leaking to other users of the pooled connection.
- **Batched jobs:** drain a backlog with `run_batched(BatchPlan { deadline, full_batch }, || batch())`. It stops when a batch comes back short or the deadline passes, and `BatchedRun::into_job_result` reports what remains for the next run.
- **Once per release:** `ReleaseStampRepository::run_once(ReleaseRun { name, release: env!("CARGO_PKG_VERSION"), gate, wait }, work)` runs `work` under a cluster-wide lock when this release is ahead of the recorded one (`ReleaseGate::IfBehind`) or not behind it (`ReleaseGate::UnlessNewer`), and records the release only when `work` succeeds. A binary older than the recorded release never runs it, so a lagging pod in a rolling update cannot redo a newer release's work.
- **Secrets:** `ctx.secrets().secret("my_api_key")` returns a custom secret from the platform secrets document (any configured secrets source, including reloads) as an `ExtensionSecret` that redacts itself in `Debug`. Core credentials such as the pepper and database URLs are never handed out. Do not read credentials or `HOSTNAME` from the environment; `ctx.instance_id()` is the node identity.
- **Readiness:** override `Extension::readiness(ctx)` to return named `ReadinessCheck`s. Every `/readyz` runs them; a check that returns `ReadinessFailure`, errors, or overruns the probe timeout answers 503 with the failing check under `extensions`, so the probe fails closed. An `Unready` reason is shown; a `Failed` cause is only logged.
- **Metrics:** emit through the `metrics` facade and declare each metric in `Extension::metrics()` as a `MetricDescriptor`. A histogram's buckets register before the recorder installs, so it renders as buckets rather than a summary, and every descriptor's help text appears on `/metrics`.

## How discovery works

There is no runtime loader and no plugin file format. `register_extension!` expands to `inventory::submit!`, which places the registration in a link-time collection. At startup the runtime iterates `inventory::iter` over those registrations, instantiates each extension, validates that every declared dependency is also registered and that the dependency graph has no cycle, and orders the result so every dependency precedes its dependents, with ties broken by `priority` (default `100`) and then id. That one order drives schema installation, route and job collection, and the `provide` pass. Because discovery is compile-time, an extension only participates if its crate is compiled into the running binary.

## Verify

```bash
cargo build -p <your-extension-crate>
cargo run -p systemprompt --example extension --features core
```

Build the host binary and verify that startup discovers the extension, accepts its dependencies and finds every type its `requires` declares. Compilation alone does not verify runtime initialization.

## Related pages

- [concepts/extensions.md](../concepts/extensions.md) — why the framework is compile-time and how the pieces fit together.
- [configure-providers.md](configure-providers.md) — configuring AI providers through the gateway.
