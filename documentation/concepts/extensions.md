# The extension model

How systemprompt-core lets downstream projects add capabilities — schemas, routes, jobs, commands and seams — as composition members compiled into the binary, admitted against the profile before any of their code runs.

systemprompt-core is a library you compile into your binary, not a framework you drop plugins into. An extension is a Rust type that implements the `Extension` trait; the binary's `main` composes it with the core members. There is no runtime plugin loader, no `dlopen` and no registration macro: a member contributes because `main` names it.

## The `Extension` trait

```rust
pub trait Extension: Send + Sync + 'static {
    fn metadata(&self) -> ExtensionMetadata;
    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError>;
}
```

`contribute` is the one declaration. It receives a `Contributions` bound to the member's own sink and only stores: schemas, migrations, seeds, route sets, gateway routes, commands, jobs, retention, projections, run origins, user purges, provided and required state, slot entries, audience resolvers, readiness checks, diagnostics, metrics, gateway guards, site auth, configuration sections and assets. Provide fns, factories, slot builds and asset closures are deferred; nothing a member declares runs before admission. The step-by-step guide is [authoring extensions](../guides/authoring-extensions.md).

## The composition

A binary is a `Composition`:

```rust
fn main() -> Result<ExitCode, RunError> {
    reference().with(MyExtension).run()
}
```

`reference()` is every core member the build links, in dependency order, each with **core** standing and the terms the kernel's conferral table grants it. `with` adds a member with **install** standing; `without` removes an optional core member. There is no API that replaces a member, configures a core member's terms or confers anything on an install.

Composition runs in two phases:

1. **Declare.** Every member's `contribute` runs once, before any profile is loaded. Ids are validated (an install may not take a core member id), commands are admitted against the reserved CLI paths, and the CLI builds its command tree from the result.
2. **Admit.** Once the profile is read, every contribution of a member listed in `extensions.disabled` is dropped, then routes are admitted against the reserved paths and privileged shapes, and providers are checked. An unknown disabled id, a disabled required member, an enabled member depending on or requiring a disabled one, and two providers of one type fail boot with a `CompositionError`.

The runtime then resolves the **kernel** over the admitted members: it seeds the kernel's own values (instance id, runtime configuration, services view and root, secret store, system admin, event router and the document handles), runs every admitted provide fn in composition order, builds the audience resolvers, builds each slot once, and checks every declared requirement. A missing requirement is a typed startup error, never a failure on a request.

## What admission enforces

The kernel's admission rules bound what a member can declare:

- **Reserved paths.** `RESERVED_PATHS` names every API prefix and path a core member owns (OAuth, users, agents, MCP, gateway, admin, documents, brain, notifications, content, analytics, the probes, `/.well-known`, `/` and others), whether or not that member is composed. An install route reaching one is refused at admission and re-checked at mount. An install's base is under `/api/` and it has no root fallback.
- **Privileged shapes.** A handler-verified policy, an above-default body limit, a base outside `/api/` and a reserved CLI path are held only by the core member `PRIVILEGED_SHAPES` or `RESERVED_CLI_PATHS` names, and only in the listed shape.
- **Single-owner state and slots.** Each type has one provider; a slot is built once by the kernel from its entries; a type the kernel inserts cannot be provided by any member; an install value cannot satisfy a core requirement or replace a core-only seam.
- **Table ownership.** The schema installer records the owner and standing of every table it creates. A member declaring another member's table is refused before any DDL, including a table of a disabled member.
- **Typed purge.** A user purge is declared per member as a table, a column and an optional typed predicate over the member's own tables, never SQL text. Declarations are recorded, so a user purge also covers the tables of a member the profile disables.
- **Command seal.** A member's command resolves state through a resolver that refuses the kernel's configuration, secrets and application context, and receives a kernel grant only on a reserved CLI path its core owner holds.

These rules govern declarations and member commands. They do not isolate in-process code: an install member is Rust code linked into the binary, and its routes, jobs and factories run with the process's privileges.

## Typed state

The platform has one type-keyed map, `Dependencies`, used for the kernel state, job contexts, config-plane contexts and prerender contexts alike. A route set's state type implements `FromState` and is built from the resolved state; a provide fn and a factory read it through `Resolved` (`get`, `optional`). The finished state is immutable after boot. This is how a member builds a repository, service or cache once and shares it between its routes, jobs and seams without a global.

Jobs read the host's `ExtensionContext` from their context. It hands a job the profile, the resolved configuration, the services view and root, the database handle, file storage, custom secrets, background tasks, the instance id, a token authority, provider tokens and the cross-replica coordination seams (instances, leases, advisory locks).

## Why composition, not discovery

Because a member contributes only when `main` composes it, the binary is the unit of composition: adding a member means rebuilding. A domain crate never needs a compile-time dependency on another domain crate to reach its capability — the owning crate defines a slot in its `contract` module, and the composition root wires the members — which is why the dependency graph has no cross-domain edges (see [architecture.md](architecture.md)).

## Schema and migration embedding

A member owns its database schema, and the schema travels inside the crate so it works when the crate is published to crates.io.

- **Schema DDL** lives in `{crate}/schema/*.sql` and is embedded with `include_str!` in the member's `contribute`. A missing file is a compile error.
- **Migrations** live in `{crate}/schema/migrations/NNN_<name>.sql`. A one-line `build.rs` calls `systemprompt_extension::build::emit_migrations()`, which discovers those files at build time; the member contributes them with `c.migrations(extension_migrations!())`. Version and name are derived from the filename, and `cargo:rerun-if-changed` retriggers the build when a file appears.

> The extension framework's `build.rs` helper is the one sanctioned exception to the shared layer's no-I/O rule: it performs file I/O only at compile time and is never reached by any runtime code path.

The installer runs over the admitted composition in composition order, so a dependency's tables exist before its dependents' migrations run. A disabled member's schema is not installed or migrated; its recorded tables are kept.

## See also

- [authoring extensions](../guides/authoring-extensions.md) — the step-by-step guide to building one.
- [architecture.md](architecture.md) — how members keep the dependency graph acyclic and cross-domain-free.
