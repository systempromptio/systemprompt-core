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

- **Reserved paths.** `RESERVED_PATHS` names every API prefix and path a core member owns (OAuth, users, agents, MCP, gateway, admin, documents, brain, notifications, content, analytics, the probes, `/.well-known`, `/` and others), whether or not that member is composed. An install route reaching one is refused at admission and re-checked at mount. An install's base is its own `ApiBase::Extension` path under `/api/` or the web root (`WebBase::Root`); a closed core API base and `/track/engagement` are refused. At the web root an install's routes are protected route by route and it has no root fallback (`service("/")`, `/{*path}` and `/{slug}` are refused): the tree's one fallback is the kernel's static content, which a declared site auth covers whole.
- **Privileged shapes.** A handler-verified policy, an above-default body limit, a closed core base, `/track/engagement`, a gateway path and a reserved CLI path are held only by the core member `PRIVILEGED_SHAPES` or `RESERVED_CLI_PATHS` names, and only in the listed shape.
- **Single-owner state and slots.** Each type has one provider; a slot is built once by the kernel from its entries; a type the kernel inserts cannot be provided by any member; an install value cannot satisfy a core requirement or replace a core-only seam.
- **Table ownership.** The schema installer records the owner and standing of every table it creates. A member declaring a table another member creates or is recorded for is refused before any DDL, including the recorded table of a disabled member. A pending migration is judged before any DDL too: one that re-creates another member's table, or alters, drops, truncates, renames, moves, grants on, or attaches a rule or policy to it, or writes into the kernel's ledger tables, is refused, and so is an install migration's schema-level statement over a shared schema, drop of another member's index or object, or SQL the parser cannot attribute that no install ships (`CALL`, `EXECUTE`, `COPY` from a program, an unclassified statement); a `DO` block is logged as unjudged. The rule covers those statement kinds only; the guide lists what it logs instead of refusing. A member disabled before its first recorded install has no record until it is next enabled; run one install with every member enabled after upgrading.
- **Typed purge.** A user purge is declared per member as a table, a column and an optional typed predicate over the member's own tables, never SQL text. Declarations are recorded, so a user purge also covers the tables of a member the profile disables.
- **Command seal.** A member's command resolves state through a resolver that withholds the kernel's sealed values (the configuration, the secret store, the JWT signing keys, the system admin, the active services root, the services bundle sources, the event router and, in the CLI, the loaded profile) and holds no secrets sources, boot context or application context; it receives a kernel grant only on a reserved CLI path its core owner holds, and reaches the profile, a session token, the services tree or the exports directory only through a grant listed on that path's row. A value a member's own provide fn builds from a sealed value is not withheld.

These rules govern declarations and member commands. They do not isolate in-process code: an install member is Rust code linked into the binary, and its routes, jobs and factories run with the process's privileges.

## Typed state

The platform has one type-keyed map, `Dependencies`, used for the kernel state, job contexts, config-plane contexts and prerender contexts alike. A route set's state type implements `FromState` and is built from the resolved state; a provide fn and a factory read it through `Resolved` (`get`, `optional`). The finished state is immutable after boot. This is how a member builds a repository, service or cache once and shares it between its routes, jobs and seams without a global.

A job, a runner service and a config plane resolve the kernel's open seeds by type from their context: the database pool, the paths, the services view, the instance id, file storage, background tasks and the cross-replica coordination seams (instances, leases, advisory locks), plus every admitted member's provided values. The sealed seeds (the resolved configuration, the secret store, the JWT signing keys, the system admin, the services root and the event router) are withheld from all three and from a member's command, and none of them carries the secrets document or the profile; a provide fn, a factory, a slot build and a route set read them. Every one of these contexts is built from the kernel's runtime state (`Kernel::runtime_state`, the resolved state with the sealed set withheld), so a type is withheld everywhere by being sealed once. A member that needs a secret- or key-derived capability in runtime code builds a narrow, purpose-typed handle from the sealed seed in its provide fn or factory (the identity member's `IdentityClientSecrets` resolves only declared client secrets; the agent-runs runner service binds the signing keys into its run-credential issuer when its factory builds it) and its runtime code resolves that handle by type (the generator member provides `PrerenderSettings` and the agent-runs member `AgentRunRetention`, both narrowed from the sealed configuration). A member value that acts with standing authority is provided sealed (`Contributions::provide_sealed`, `provide_sealed_after_governance`) and is withheld exactly like a sealed seed: from every job, runner-service and config-plane context, every member command and the route state of every member but its provider; the kernel and provide-phase code read it. The `managed_resources` member provides `DynInventoryPublisher` (publishes as the system admin) and `DynManagedSkillResolver` (resolves for a caller-named owner) sealed. `Job::configured` reads a deployment map that holds only the scheduler member's typed facts for its own jobs; it carries no profile.

A job name has one owner. Two admitted members contributing the same name are refused at composition (`CompositionError::DuplicateJob`) and again when the kernel builds the schedule (`DuplicateJobOwner`), before any schedule row is written; the scheduler's kernel jobs are recognised by their admitted owner's stamped core standing, never by name.

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
