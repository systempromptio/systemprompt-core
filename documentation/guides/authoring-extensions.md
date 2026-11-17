# Authoring an extension

How to build a systemprompt extension: a Rust type that declares its schema, routes, jobs, commands and seams, composed into the binary with the core members.

An extension is a composition member: a type that implements `Extension` and is added to the binary's `Composition`. There is no runtime plugin loading, no dynamic library, no registration macro and no manifest file to ship; a member contributes only when the binary's `main` composes it. For the design, see [concepts/extensions.md](../concepts/extensions.md).

## Prerequisites

- A Rust toolchain matching the workspace edition (Edition 2024; minimum Rust per `rust-toolchain.toml`).
- A crate that depends on the `systemprompt` facade with the `full` feature. `Composition` and `reference()` are compiled only with `full`; a library crate that only declares a member can depend on `systemprompt-extension` (with its `router` and `cli` features for route sets and commands).

```toml
# Cargo.toml
[dependencies]
systemprompt = { version = "0.83", features = ["full"] }
```

The complete minimal member and binary is the facade example `systemprompt/examples/extension.rs`:

```rust
use std::process::ExitCode;

use async_trait::async_trait;
use axum::http::Method;
use systemprompt::extension::cli::{
    CliCommandGroup, CommandDecl, CommandDescriptor, CommandError, CommandHost, CommandOutput,
    DescribeCommand,
};
use systemprompt::extension::prelude::*;
use systemprompt::extension::router::{ApiBase, BasePath, BodyLimit, RouteLimits, RouteSet, RouteSpec};
use systemprompt::extension::RouteGroup;
use systemprompt::traits::{Job, JobContext, JobResult, ProviderResult};
use systemprompt::{RunError, reference};

const LIMITS: RouteLimits = RouteLimits {
    rate: RateLimit::new("demo", RateBudget::Contexts),
    body: BodyLimit::Default,
};
const USER: RouteSpec = RouteSpec::new(AuthzPolicy::user(), ContextKind::User, LIMITS);

#[derive(Debug, Clone, Copy, Default)]
struct DemoExtension;

impl Extension for DemoExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "demo-extension",
            name: "Demo Extension",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        c.schema(SchemaDefinition::new(
            "demo_extension_state",
            "CREATE TABLE IF NOT EXISTS demo_extension_state (id TEXT PRIMARY KEY)",
        ))
        .routes(
            RouteSet::at::<DemoState>(
                BasePath::Api(ApiBase::Extension("/api/demo")),
                RouteGroup::Extensions,
            )
            .route(Method::GET, "/status", status, USER)
            .build(),
        )
        .job(DemoJob)
        .command(CommandDecl::group::<DemoCommands>());
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct DemoState;

impl FromState for DemoState {
    fn from_state(_r: &Resolved<'_>) -> Result<Self, MissingDependency> {
        Ok(Self)
    }
}

async fn status() -> &'static str {
    "ok"
}

#[derive(Debug, Clone, Copy)]
struct DemoJob;

#[async_trait]
impl Job for DemoJob {
    fn name(&self) -> &'static str {
        "demo_heartbeat"
    }

    fn schedule(&self) -> &'static str {
        "0 0 * * * *"
    }

    async fn execute(&self, _ctx: &JobContext) -> ProviderResult<JobResult> {
        Ok(JobResult::success())
    }
}

#[derive(Debug, Clone, clap::Subcommand)]
enum DemoCommands {
    Status,
}

impl DescribeCommand for DemoCommands {
    fn descriptor(&self) -> CommandDescriptor {
        CommandDescriptor::NONE
    }
}

impl CliCommandGroup for DemoCommands {
    const NAME: &'static str = "demo";
    const ABOUT: &'static str = "Demo extension commands";

    async fn run(self, _host: &dyn CommandHost) -> Result<CommandOutput, CommandError> {
        match self {
            Self::Status => Ok(CommandOutput::text("demo extension is installed")),
        }
    }
}

fn main() -> Result<ExitCode, RunError> {
    reference().with(DemoExtension).run()
}
```

```bash
cargo run -p systemprompt --example extension --features full -- demo status
```

## The `Extension` trait

`Extension` has two methods:

```rust
pub trait Extension: Send + Sync + 'static {
    fn metadata(&self) -> ExtensionMetadata;
    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError>;
}
```

`ExtensionMetadata` has three `&'static str` fields: `id` (validated into an `ExtensionId` once, at composition), `name` and `version`.

`contribute` runs once per composition, before the profile is loaded, and only stores declarations into the member's own sink. No method of `Contributions` accepts or returns another member's entries. Provide fns, factories, slot builds and asset closures are deferred: nothing a member declares runs until the composition has admitted it against the profile. A declaration that fails validation (an invalid base path, a route set built through the wrong builder) refuses the composition with `CompositionError::Contribution` naming the member.

## Identity and standing

A member's identity is its Rust type and its standing, not a string it supplies. Every member a binary adds with `Composition::with` has **install** standing; the core members in `reference()` have **core** standing. There is no API that confers core standing, a reserved path, a privileged route shape or a kernel grant on an install member, and no API that replaces a core member.

An install member may not take a core member id (`CompositionError::ReservedId`), whether or not that core member is linked or enabled; `systemprompt::CORE_MEMBER_IDS` is the list. Two members with one id are refused (`DuplicateId`).

Tables, purge records and migration ledgers are recorded against the member id. Changing a member's id is refused against tables recorded to the old id (`LoaderError::DuplicateTableOwner` names the owner) until the records are re-assigned.

## What a member contributes

Every method on `Contributions` returns `&mut Self` and chains.

### Schemas, migrations and seeds

- `c.schema(SchemaDefinition::new(table, sql))` declares a table. Embed the DDL from `{crate}/schema/<table>.sql` with `include_str!`. The tables a member owns are the tables its schema SQL creates (`CREATE TABLE`), not the `table` label; schema SQL that does not parse refuses admission.
- `c.migrations(extension_migrations!())` returns the migrations `build.rs` discovers (see [Migrations](#migrations)).
- `c.seed(Seed)` declares seed data; `c.retire(SchemaDefinition)` declares a table the member no longer creates.
- `c.cross_table(table)` names a table another member creates that this member's migrations may alter, drop, truncate or rename. An install's declaration does not cover a core member's table. Do not list owned tables there.

The installer runs over the admitted composition and records the owner and standing of every table it creates in `schema_table_owners`. A member declaring a table recorded to another member is refused before any DDL runs, including a table recorded to a member the profile disables, and an install claiming a table recorded under core standing is refused (`LoaderError::CoreRecordClaimedByInstall`).

Before any DDL the installer also judges every migration that will run (one not yet recorded as applied, on a database that is not fresh): an `ALTER TABLE`, `DROP TABLE`, `TRUNCATE` or `RENAME` of a table another member creates or is recorded for is refused (`MigrationOwnershipError::ForeignTable`, naming the member, the statement and the table), unless a `cross_table` declaration covers it. A foreign-key `REFERENCES`, a read and a data write are not judged. A `DO` block hides the tables it mutates, so a pending install migration holding one is refused (`MigrationOwnershipError::Unreadable`); a core migration's is logged. A member disabled before its first recorded install has no ownership record until it is next enabled, so after upgrading run one install with every member enabled.

### Routes

A member's HTTP surface is one or more `RouteSet`s, contributed with `c.routes(set)`. A set has a base, a route group, a state type and a spec per route:

```rust
RouteSet::at::<DemoState>(
    BasePath::Api(ApiBase::Extension("/api/demo")),
    RouteGroup::Extensions,
)
.route(Method::GET, "/status", status, USER)
.build()
```

- The state type implements `FromState` and is built from the resolved state once the composition is admitted; a missing dependency fails boot.
- Each route carries a `RouteSpec`: an `AuthzPolicy` (`public()`, `authenticated()`, `user()`, `admin()`, …), a `ContextKind` and `RouteLimits` (a `RateLimit` scope and budget, and a `BodyLimit`). The API mount applies each admitted set's policy, context, rate limit, body limit and protection layers; nothing is applied that the spec does not name.
- `.service(prefix, service, spec)` nests one opaque service (a `tower` service) under a literal prefix of the set. It is recorded as one `ANY` route that conflicts with every other route the prefix covers.
- Two routes with one method and path, across all members, refuse the composition.

**Bases.** An install member's base is `ApiBase::Extension(path)`, where the path starts with `/api/`, contains no `.` and does not cover a reserved path (`ContributionError::InvalidBase`), or the web root (`BasePath::Web(WebBase::Root)`). A closed core API base is refused (`CompositionError::NotConferred`), as is `/track/engagement`. A set at the web root is protected route by route, so its gate never answers a path it does not route.

**Reserved paths.** Every route's full path, parameter, catch-all and `service` prefix is checked at admission, and re-checked when the server mounts it, against `systemprompt_extension::reserved::RESERVED_PATHS`: discovery, `/api/v1/auth/me`, the meta, OAuth, users, agents, MCP, stream, files, scheduler, core, admin, brain, documents, document folders, uploads, internal, issues, marketplace, sync, webhook, health and consumer API prefixes, the gateway (`/v1` and `/api/public/gateway`), `/.well-known`, `/`, `/health`, `/livez`, `/readyz`, `/auth/link-passkey`, the content base and `/r`, the analytics base and `/track`, notifications and admin notifications, agent runs, Slack, Teams and the cloud tenants path. Each is owned by one core member, whether or not that member is composed. A route reaching one is refused (`CompositionError::ReservedPrefix`).

**No root fallback.** An install cannot serve a catch-all at the root: `service("/")`, `/{*path}` and `/{slug}` are refused, and one opaque `service` cannot cover all of `/api/public`. Serve a fallback under an explicit prefix of the member's own base. The tree's one fallback is the kernel's static content: it is layered whole, so a declared site auth and the public context cover every page it serves, and a second claim on it refuses boot (`MountError::SecondRootFallback`).

**Privileged shapes.** A handler-verified policy, the `HandlerResolved` context, a body limit above the server default, a closed core base, `/track/engagement`, a gateway path and a reserved CLI path are held only by the core member that `PRIVILEGED_SHAPES` or `RESERVED_CLI_PATHS` names, and only in exactly the listed shape. An install member using one is refused at admission, and the mount re-checks every admitted entry that uses a privilege.

Gateway routes (`c.gateway_route(GatewayRouteDecl)`, built with `RouteSet::gateway`) are held only by the core member a gateway path is listed for. Each is admitted against the built-in gateway prefixes and the others (`GatewayRouteShadowsBuiltin`, `GatewayRouteOverlap`); every route of the set lies within its claimed gateway path (`GatewayRouteOutsideClaim`) and under no built-in gateway prefix (`GatewayRouteUnderBuiltin`), and two gateway routes claiming one request conflict (`RouteConflict`). The mount re-checks each rule. Admitted gateway routes keep the gateway credential, bearer window and access log.

### Jobs

`c.job(job)` contributes a scheduled job (a `Job` implementation with a `name` and a cron `schedule`). The scheduler runs only the jobs of admitted members, read from `AdmittedSchedule`. Retention declarations (`c.retention`), job-run scopes (`c.job_run_scope`) and projection drains (`c.projection`) are admitted the same way.

A job's `JobContext` reads the host context with `ctx.get::<Arc<dyn ExtensionContext>>()` and provided values with `ctx.get::<T>()`; see [Runtime seams](#runtime-seams).

### Commands

`c.command(CommandDecl::group::<T>())` contributes a CLI group (`CommandDecl::group_under` and `leaf_under` attach below an existing group). The type implements `clap::Subcommand`, `DescribeCommand` and `CliCommandGroup`; its `run(self, host: &dyn CommandHost)` returns a `CommandOutput` that the CLI renders once (text, or JSON under the global `--json`).

A member command:

- needs a profile, unless it declares `DbUrlRouting::Direct`: on a bare `--database-url` run (no profile) a `Direct` command runs over every linked member with none disabled, whatever its standing, and any other member command is refused (`ProfileRequired`);
- reads state through `host.resolved().await`, which withholds the kernel's sealed values (the configuration, the secret store, the system admin, the active services root and the event router) and holds no secrets sources, environment overrides, boot context or application context. A value a member's own provide fn builds from a sealed value is not withheld, so an install that wraps one hands it to its commands;
- may request kernel grants with `CommandDecl::requesting` only on a reserved CLI path its core owner holds (`admin mcp`, `analytics`, `cloud`). An install member holds no reserved CLI path, so a grant request by an install is refused (`CompositionError::GrantNotConferred`), as is a command at a reserved path (`ReservedCliPath`) or a path declared twice.

### User purge of the member's own tables

A member that owns user-keyed tables declares how a user's rows leave them:

```rust
c.user_purge(UserPurgeTable { table: "demo_items", column: "user_id", predicate: None })
    .orphan_sweep(OrphanSweep {
        table: "demo_payloads",
        key: "sha256",
        referenced_by: "demo_items",
        via: "payload_sha256",
        user_column: "user_id",
    });
```

A `PurgePredicate` (`{ column, any_of }`) narrows a table to rows whose column holds one of a fixed set of values. It is not SQL: the column is rendered as a quoted identifier and the values are bound. A purge table, and both tables of an orphan sweep, are admitted only over the declaring member's own schema tables, whatever its standing; anything else is refused when the kernel resolves (`KernelError::Purge`) and at schema install (`LoaderError::PurgeAdmission`). The installer records each member's declarations in `schema_purge_records`.

### Configuration

`c.config::<C>()` declares a typed configuration section: `C` implements `ExtensionConfig` (`serde::Deserialize` + `schemars::JsonSchema`, a `PREFIX` and `validate`). Startup validation reads `<services>/config/<PREFIX>.yaml` (an absent file validates as `{}`) and reports a malformed section under `<PREFIX>.config`. `c.config_plane(plane)` contributes a config plane reconciled against the services tree.

### Shared state

`c.provide::<T, _>(|r| …)` provides a value of type `T` built from the resolved state; `c.requires::<T>()` declares a type the member cannot run without; `c.depends_on::<E>()` orders the member after `E`. A provide fn reads dependencies with `r.get::<T>()` (a missing one fails boot) or `r.optional::<T>()`. Each type has exactly one provider: two members providing one type are refused (`DuplicateProvider`), and a type the kernel inserts (the audience resolvers, subject provider set, policy catalog and gateway guards) cannot be provided or slot-built by any member (`KernelError::ReservedType`). An install-provided value cannot meet a core member's requirement (`InstallSubstitution`).

Readiness checks (`c.readiness`), diagnostic probes (`c.diagnostic`), metric descriptors (`c.metric`), runner services (`c.runner_service`), gateway guards (`c.gateway_guard`), subject attribute providers (`c.subject_attributes`), governance policies (`c.policy`), site auth (`c.site_auth`), roles (`c.role`), run origins (`c.run_origin`) and assets (`c.asset_with`) are contributed the same way, each as a factory or value the kernel builds after admission.

### Slots

A capability seam is a slot its owning crate defines in a `contract` module; a member contributes an entry with `c.slot::<S>()`, which returns the slot's builder. The kernel builds each slot once. The slots are `SsoSlot` (oauth-issuance), `ToolsetSlot` (mcp-host), `RouteSelectorSlot`, `OverrideSlot`, `SafetyScannerSlot` and `OutboundSlot` (gateway), `AuthzHookSlot` and `RoleDirectorySlot` (governance), `BrainSlot`, `NotificationsSlot`, `MarketplaceSlot`, `UiSlot` and `AccessRecorderSlot` (mcp), `ContentSlot`, and `BlobHolderSlot`, `AgentAccessSlot` and `DocumentsSlot` (files).

A failing entry refuses the slot build (`SlotError::Entry`, `SlotError::Refused`); it is never skipped. An install entry cannot replace the gateway safety scanners, authz hooks, route selectors or overrides, and an outbound adapter or safety scanner named like a built-in, a second artifact theme, two renderers for one artifact type, a second role directory or a second agent access policy refuses the composition.

### Audience resolvers

`c.audience_resolver(factory)` contributes a `NotificationAudienceResolver` to the one `AudienceResolvers` set the notification pipeline and brain recall both read. Core alone resolves the `User`, `Role` and `Everyone` subjects and the reserved subject dimensions: an install resolver is for a custom dimension only. One claiming a reserved subject is refused (`SlotError::ReservedAudience`), every install resolver sits behind a guard that never answers a reserved subject, and two resolvers with one name are refused.

## What an install member may not do

Admission refuses, before anything the member declares runs:

- a core member id, or a duplicate id;
- a route reaching a reserved path, a root fallback, or a base other than its own `/api/` extension base or the web root;
- a privileged route shape, a reserved CLI path or a kernel grant;
- a table, purge table or orphan sweep over a table it does not create, or a table recorded to another member;
- a pending migration altering, dropping, truncating or renaming another member's table, or holding a `DO` block;
- a second provider of a type, a type the kernel inserts, or a value substituting for a core requirement;
- an audience resolver for a reserved subject or dimension, or a subject provider declaring `user`, `role` or `traffic`;
- a slot entry that replaces a core-only seam.

These rules bound what a member can **declare**, and what its **commands** can read. They are not a sandbox: an install member is Rust code linked into the same process, with the same memory and system access as core.

## Composing the binary

The binary's `main` is the composition:

```rust
fn main() -> Result<ExitCode, RunError> {
    reference().with(DemoExtension).run()
}
```

- `reference()` is every core member this build links, in dependency order.
- `.with(member)` adds a member with install standing; call it once per member.
- `.without::<CoreMember>()` removes an optional core member; a required one cannot be removed.
- `.secrets_source(kind, factory)` declares an external secrets source; a kind registered twice, or reserved for core, is refused.
- `.run()` calls every member's `contribute`, admits commands against the reserved CLI paths, builds the command tree and starts the CLI; once a profile is loaded the composition is admitted against it, and `infra services serve` starts the server over the admitted members.

## Disabling a member in a profile

```yaml
extensions:
  disabled:
    - demo-extension
```

A disabled member's contributions are dropped at admission, before any provide fn, factory, slot build or asset closure runs. Its `contribute` still runs, since that is how its declarations are read, and only stores:

- none of its contributions runs: no routes are mounted, no commands are offered, no jobs, retention, guards, seams or providers are built;
- its job names are unknown: a scheduler config naming one is refused at validation, `admin config validate` reports it as not contributed, and `infra jobs prune` treats its rows as prunable;
- its recorded tables are kept: the schema installer does not drop them, no other member can claim them, and no other member's pending migration can alter, drop, truncate or rename them. A member disabled before its first recorded install has no record until it is next enabled;
- a user purge still covers them, through the purge declarations recorded when the member was last installed.

An id in `extensions.disabled` that names no composed member fails boot, as do a disabled required member, an enabled member that depends on a disabled one, and an enabled member requiring a type only a disabled member provides.

## Migrations

Schema changes after the initial `CREATE TABLE` are migrations in `{crate}/schema/migrations/NNN_<name>.sql`, where `NNN` is a numeric version prefix. The version and name come from the filename.

1. Add a build script, with `systemprompt-extension` in `[build-dependencies]`:

   ```rust
   // build.rs
   fn main() {
       systemprompt_extension::build::emit_migrations();
   }
   ```

2. Add migration files, for example `schema/migrations/001_add_demo_items_label_index.sql`.

3. Contribute the generated list: `c.migrations(extension_migrations!())`.

| File | Effect |
|------|--------|
| `NNN_<name>.sql` | An up migration; `NNN` is the version, the remainder is the name. |
| `NNN_<name>.down.sql` | The optional paired down migration. |
| First non-blank line `-- @no-transaction` | Emitted with `Migration::new_no_transaction`, for statements Postgres rejects inside a transaction (for example `CREATE INDEX CONCURRENTLY`). A `-- @no-transaction` migration must not declare a `.down.sql`. |
| `NNN_<name>.tombstone` | Records the slot as spent. No SQL — the body is prose explaining why. |
| `NNN-MMM_<name>.tombstone` | The same, for an inclusive range of spent slots. |

The build fails if a file is misnamed or two files claim the same version. Adding a file retriggers the build through the emitted `cargo:rerun-if-changed` directive.

### Deleting a migration

**Never free a number by deleting its file.** A migration that has shipped is applied in every established database, which keeps a tracking row for that version. Refilling a freed number means the runner sees the version as applied, skips the new SQL, and refuses to boot on a checksum belonging to a migration nobody can find.

Deleting is two steps: remove the `.sql` and leave a `.tombstone` in its place, keeping the original name.

```
schema/migrations/
  007_add_index.sql          ->  007_add_index.tombstone
  008_next_change.sql
```

The runner never executes, records or checksums a tombstoned slot, and `emit_migrations` treats the number as occupied, so reusing it fails the build rather than a deployment. `infra db migrate-status` lists any applied version that no file claims as `orphaned`.

## Runtime seams

Core owns these contracts so a member never reimplements them against core tables. `ctx` below is the `ExtensionContext` a job reads with `ctx.get::<Arc<dyn ExtensionContext>>()`.

- **Governance:** build a `PolicyRequest` and call `GovernanceEngine::charge_and_evaluate`. The engine charges every armed rate window before the chain runs.
- **Replica liveness:** `ctx.instances().is_alive(&instance_id)` is true while that replica holds its instance claim or heartbeats its `services` rows within 90 seconds. `ctx.instance_id()` is this node's identity.
- **Leases:** `ctx.leases().claim(&LeaseKey::new("…"), ttl)` grants an expiring single-owner claim on a named resource, or `None` while another live replica holds it. A dead owner's lease is forfeit even before it expires.
- **Advisory locks:** `ctx.advisory_locks().try_acquire(&AdvisoryLockKey::new("…"))` (or `acquire_within(key, wait)`) takes a cluster-wide lock on the session pool. Call `release().await` on the guard; dropping it closes the locking session, which also frees the lock.
- **Job database budgets:** override `Job::db_budget` to return a `DbBudget`, then open the job's transactions with `ctx.get::<BudgetedDb>()?.begin()`. The budget is applied with `SET LOCAL`.
- **Batched jobs:** drain a backlog with `run_batched(BatchPlan { deadline, full_batch }, || batch())`; `BatchedRun::into_job_result` reports what remains for the next run.
- **Once per release:** `ReleaseStampRepository::run_once(ReleaseRun { name, release: env!("CARGO_PKG_VERSION"), gate, wait }, work)` runs `work` under a cluster-wide lock when this release is ahead of the recorded one (`ReleaseGate::IfBehind`) or not behind it (`ReleaseGate::UnlessNewer`), and records the release only when `work` succeeds.
- **Secrets:** `ctx.secrets().secret("my_api_key")` returns a custom secret from the platform secrets document as an `ExtensionSecret` that redacts itself in `Debug`. Do not read credentials or `HOSTNAME` from the environment.
- **Readiness:** a check contributed with `c.readiness` runs on every `/readyz`; a check that fails, errors or overruns the probe timeout answers 503 with the failing check under `extensions`.
- **Metrics:** emit through the `metrics` facade and declare each metric with `c.metric(MetricDescriptor)`, so a histogram renders as buckets and its help text appears on `/metrics`.

## Upgrading from 0.82

- Move each defaulted hook body (`schemas`, `migrations`, `routers`, `jobs`, `provide`, `requires`, the seam hooks) into `contribute`, and replace each `register_*!`/`submit_*!` registration with the matching `Contributions` method or the owning crate's `contract` slot. Drop the `inventory` dependency.
- Replace `ExtensionRouter` with `RouteSet`s under the member's own `/api/` base or the web root, and any root fallback with an explicit prefix.
- Move command groups to `run(&dyn CommandHost) -> CommandOutput`.
- Replace the binary's `main` with `reference().with(..).run()`.
- Run one install with every member enabled, so table ownership and purge declarations are recorded for each member. A profile with disabled members runs one full schema install on its next boot.

## Verify

```bash
cargo check -p <your-extension-crate>
systemprompt plugins list
systemprompt plugins seams --member <id>
```

`plugins list` and `plugins seams` report the admitted composition member by member. Compilation alone does not admit a member: run the binary against a profile to see admission accept it.

## Related pages

- [concepts/extensions.md](../concepts/extensions.md) — the composition contract and how the pieces fit together.
- [configure-providers.md](configure-providers.md) — configuring AI providers through the gateway.
