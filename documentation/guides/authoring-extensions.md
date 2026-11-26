# Authoring an extension

How to build a systemprompt extension: a Rust type that declares its schema, routes, jobs, commands and seams, composed into the binary with the core members.

An extension is a composition member: a type that implements `Extension` and is added to the binary's `Composition`. There is no runtime plugin loading, no dynamic library, no registration macro and no manifest file to ship; a member contributes only when the binary's `main` composes it. For the design, see [concepts/extensions.md](../concepts/extensions.md).

## Prerequisites

- A Rust toolchain matching the workspace edition (Edition 2024; minimum Rust per `rust-toolchain.toml`).
- A crate that depends on the `systemprompt` facade with the `reference` feature. `Composition` and `reference()` are compiled only with `reference`; a library crate that only declares a member can depend on `systemprompt-extension` (with its `router` and `cli` features for route sets and commands).

```toml
# Cargo.toml
[dependencies]
systemprompt = { version = "0.87", features = ["reference"] }
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
cargo run -p systemprompt --example extension --features reference -- demo status
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

Before any DDL the installer also judges every migration that will run (one not yet recorded as applied, on a database that is not fresh) and every statement of an install member's declarative schema, which runs on every full install (an install whose schema is settled runs no DDL and judges nothing). A schema statement is judged exactly as a migration statement is (the finding names `schema` in place of the migration); a core member's declarative schema is not judged. A table belongs to its live ownership record, then to the member whose declarative schema creates it, then to the member whose migrations create it and do not drop it again.

- A `CREATE TABLE`, with or without `IF NOT EXISTS`, `CREATE TABLE ... AS` or `SELECT ... INTO` of a table another member owns is refused (`MigrationOwnershipError::SharedTable`, naming both members and the table).
- `ALTER TABLE` (including `SET SCHEMA` and `ENABLE ROW LEVEL SECURITY`), `DROP TABLE`, `TRUNCATE`, `RENAME` of a table, column, constraint, trigger, policy or rule, `CREATE RULE`, `CREATE POLICY`, `ALTER POLICY`, `DROP POLICY|RULE ... ON`, and `GRANT`/`REVOKE` on another member's table are refused (`MigrationOwnershipError::ForeignTable`), unless a `cross_table` declaration covers it. Every relation a statement changes is judged, not only its main one: the parent of `CREATE TABLE ... INHERITS`, `PARTITION OF` and `ALTER TABLE ... INHERIT | NO INHERIT`, the partition of `ALTER TABLE ... ATTACH | DETACH PARTITION`, and each statement a `CREATE RULE` runs. A relation a statement only reads or constrains against (`LIKE ... INCLUDING`, a foreign-key `REFERENCES`, a query's `FROM`) is not changed by it and is not judged. A `sql`-language routine's body is read as part of the migration.
- `DROP SCHEMA`, `ALTER SCHEMA ... RENAME`, `GRANT`/`REVOKE ... ON SCHEMA` and `GRANT ... ON ALL ... IN SCHEMA` over `public` or a schema holding another member's objects, and `ALTER TABLE ALL IN TABLESPACE`, are refused (`MigrationOwnershipError::SchemaLevel`).
- An `INSERT`, `UPDATE`, `DELETE`, `MERGE` or `COPY` into a kernel ledger (`extension_migrations`, `extension_migration_compat`, `schema_table_owners`, `schema_object_owners`, `schema_purge_records`, `schema_install_fingerprint`, `schema_trigger_digests`) is refused (`MigrationOwnershipError::LedgerWrite`), also inside a rule's actions, except `DELETE FROM extension_migrations WHERE extension_id = '<id>'` (or `IN (...)`) for ids no member or record holds. Every other statement on a ledger, a trigger, index, rule or policy included, is refused (`ForeignTable`) for every member but the core `database` member, and so is an install member's view, materialized view or rule that reads a ledger (a simple view over a ledger is auto-updatable, so a write into it would be a write into the ledger); an install cannot take that id (`CompositionError::ReservedId`), and an install-standing member never holds a core-standing claim even under the same id.
- Tables and objects are ranked the same way: a live ownership record first (`schema_table_owners` for tables, `schema_object_owners` for every other object), then a core member before an install member, then a declarative schema before a migration, then composition order. A table created by `CREATE TABLE ... AS` or `SELECT ... INTO` is a table. A record is not live, is no claim, and is deleted by the install after its DDL, when it carries install standing, its owner is admitted and not held back in this install, and a core member's SQL creates the same table or object (the owner's own create of that name is then refused as shared, so one written when an install's declarative schema outranked a core migration does not survive), or when it names an object its owner, admitted and not held back, no longer creates (a member's own record never counts as the create that keeps it; a member with a tombstoned migration keeps its object records, since a tombstone carries no SQL, except a trigger on a table its retirements name; an object such a member really dropped therefore keeps its record, and another install member creating that name is refused until the record is removed from `schema_object_owners`). A record whose owner is held back or disabled is always live: a core member declaring that table is refused (`LoaderError::DuplicateTableOwner`), and a core member creating that object is refused (`ForeignObject`), so a disabled member's recorded tables and objects cannot be adopted; the record, the rows and the absent member's purge record stay, and `infra db doctor` keeps listing the table under the absent member. An index, routine, view, materialized view, sequence, type, domain or trigger (a trigger is identified by its name and its table) belongs to the first member whose SQL creates it and does not drop it again. For an install member, a create of such an object another member owns is refused, in a migration or in its declarative schema, including `CREATE OR REPLACE`, `CREATE ... IF NOT EXISTS`, `CREATE INDEX IF NOT EXISTS` and `CREATE OR REPLACE TRIGGER`; so are `ALTER` (including `RENAME`, `OWNER TO`, a type's `ADD`, `DROP`, `ALTER` or `RENAME ATTRIBUTE` and a domain's `RENAME CONSTRAINT`), `ALTER INDEX`, `ALTER TRIGGER ... RENAME`, `DROP`, `DROP INDEX` and `DROP TRIGGER` (with or without `IF EXISTS`), `SET SCHEMA`, and `GRANT`/`REVOKE` on a routine, sequence, type or domain of such an object (`MigrationOwnershipError::ForeignObject`). The object is found by its name whatever statement form names it, so `ALTER TABLE` used to rename, move or re-own a view, materialized view, sequence or index is judged against that object's owner. Naming another member's object in a create never makes it the install's own. A member held back from an install still owns what it creates and its SQL is not judged; a disabled member owns what its live records name. Dropping or retiring a table releases the triggers on it, their records included.
- For an install member, also refused are `CALL`, `EXECUTE`, `COPY` from a file or program, a `sql` routine whose body does not parse, and any statement kind the reader does not classify, whatever it names (a `RENAME` or `GRANT` of an object kind not listed above included) (`MigrationOwnershipError::Unreadable`). A core member's statements of these kinds are logged as unjudged.

An install migration or declarative schema may contain plain DDL on its own tables (create, alter, index, drop, constraints and triggers on them), `sql`-language functions, `COMMENT ON`, `LOCK`, `REFRESH MATERIALIZED VIEW`, `VACUUM`, `REINDEX`, data writes into its own tables, and foreign-key `REFERENCES` to other members' tables. The installer logs at `warn`, naming the member and the migration, and does not refuse, a `DO` block (once per migration), `CREATE INDEX` on another member's table, a new trigger, or a drop of the member's own trigger, on another member's table (not a kernel ledger), routines in other languages, `CREATE EXTENSION`, a routine call in a top-level `SELECT` or inside an `INSERT`, `UPDATE`, `DELETE` or `MERGE`, a `DROP INDEX` of an index no member's SQL creates, and a `CASCADE` drop (every one by an install member; by a core member, one of an object it does not own); what those routines and triggers run, and data writes into another member's tables, are not judged. A table whose owner names it in its retirements and no longer declares it has no owner: its record is released, any member's migration may alter or drop it, and a member may declare it on a later install. The rule guards against mistakes in a member's migrations and declarative schema; it is not a sandbox, since a `DO` block or a routine in another language can run any statement the rule refuses. A member disabled before its first recorded install has no table or object ownership record until it is next enabled, so after upgrading run one install with every member enabled.

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

A job's `JobContext` reads the kernel's open seeds and provided values with `ctx.get::<T>()`; sealed seeds (configuration, secret store, JWT signing keys, system admin, services root, event router) are withheld, and no job, runner service or config plane can read the secrets document or the profile. A job that needs a secret- or key-derived capability, or a configuration fact, resolves a narrow handle its member builds in a provide fn (or, for a runner service, in its factory). A member value that acts with standing authority is provided sealed (`Contributions::provide_sealed`, `provide_sealed_after_governance`) and is withheld exactly like a sealed seed: from every job, runner-service and config-plane context, every member command and the route state of every member but its provider; the kernel and provide-phase code read it. The `managed_resources` member provides `DynInventoryPublisher` (publishes as the system admin) and `DynManagedSkillResolver` (resolves the system admin's managed skills for a caller-named consumer) sealed. A seal covers the capability, not one type name: no open provided value is the sealed value's implementor, wraps it or shares its allocation. The `core skills` commands read the owner's own skills through a separate open `OwnerSkillView`, which takes no consumer. `Job::configured` receives no profile: an install job that needs a deployment fact reads its member's provided value in `execute`. Job names are unique across the composition: a job named like another member's job, a scheduler job included, is refused (`CompositionError::DuplicateJob`). See [Runtime seams](#runtime-seams).

### Commands

`c.command(CommandDecl::group::<T>())` contributes a CLI group (`CommandDecl::group_under` and `leaf_under` attach below an existing group). The type implements `clap::Subcommand`, `DescribeCommand` and `CliCommandGroup`; its `run(self, host: &dyn CommandHost)` returns a `CommandOutput` that the CLI renders once (text, or JSON under the global `--json`).

A member command:

- has its place of execution and data impact projected from its descriptor: a command whose descriptor touches the database is remote-mutating (it routes to the tenant whenever the profile is a cloud profile and never falls back to direct database access) and destructive (it refuses a cloud profile that arrived implicitly) unless its declaration lists a claim with `CommandDecl::claiming` that relaxes it — `read-only-routing` (falls back to local access with a warning), `local-database` (runs against the profile's own database on this machine), `preserves-data` (destroys nothing). A claim is held only on a reserved CLI path whose row confers it, so an install command claims none and declares `DataImpact::Destructive` or `Irreversible` on every database command: the composition refuses a claim its path's row does not confer (`CompositionError::ClaimNotConferred`), and the CLI refuses a resolved descriptor making a claim its declaration does not list;
- needs a profile, unless its declaration lists the `direct-database` claim (rows `analytics`, `core content`, `core content popular`, `core files`): on a bare `--database-url` run (no profile) such a command runs over every linked member with none disabled, whatever its standing, and any other member command is refused (`ProfileRequired`). A cloud profile that reaches its database directly (`external_db_access`) is still a profiled run: admission is against that profile and a disabled member's command is refused (`CommandDisabled`);
- reads state through `host.resolved().await`, which withholds the kernel's sealed values (the configuration, the secret store, the JWT signing keys, the system admin, the active services root, the services bundle sources, the event router and, in the CLI, the loaded profile) and holds no secrets sources, environment overrides, boot context or application context. A value a member's own provide fn builds from a sealed value is not withheld, so an install that wraps one hands it to its commands;
- may request kernel grants with `CommandDecl::requesting` only on a reserved CLI path its core owner holds, and only the grants that path's row lists (`admin mcp`, `admin agent-host`: `child-host`; `analytics`: `local-export`, `skip-validation`; `cloud`: `cloud-operator`, `boot-profile`, `config-reconcile`, `target-admission`; `core marketplace`, `core plugins`, `core skills`: `services-tree`; `admin agents`: `services-tree`, `caller-session`; `core contexts`, `core artifacts`, `plugins mcp`: `caller-session`; `core content`, `core content popular`, `core files`: none). An install member holds no reserved CLI path, so a grant request by an install is refused (`CompositionError::GrantNotConferred`), as is a command at a reserved path (`ReservedCliPath`) or a path declared twice;
- reaches kernel-held material only through a grant on its path's row: `host.profile()` and `host.boot_config(path)` need `boot-profile`, `host.admitted_views(path)` needs `target-admission`, `host.export_dir()` (the profile's local exports directory) needs `local-export`, `host.services_tree()` (the active services root plus the published API URL and site GitHub link, read without a database) needs `services-tree`, and `host.caller_session()` (the CLI caller's own session: its token and the caller's own session, user and context ids, never one minted for another subject; a command presents it only to an endpoint the platform hosts) needs `caller-session`. Every other caller is refused.

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

### Owner reassignment of the member's own tables

A member whose tables key rows on a user declares how an account merge moves them, one `OwnerReassignment` per user-keyed column. A declaration is data, not code:

```rust
c.owner_reassignment(OwnerReassignment {
    table: "demo_items",
    column: "user_id",
    action: ReassignAction::UPDATE,
})
.owner_reassignment(OwnerReassignment {
    table: "demo_tokens",
    column: "user_id",
    action: ReassignAction::DELETE,
});
```

`ReassignAction::Update { touch, keep_target_on }` rewrites the column to the target user; `touch` names a timestamp column set to the current time, and `keep_target_on` names the columns that, with the user column, identify a row, so a source row whose counterpart the target already holds is left in place instead of colliding. `ReassignAction::Delete { predicate }` deletes the source user's rows (credentials or counters minted for that user), optionally narrowed by a `PurgePredicate`. `ReassignAction::ReplaceInArray` replaces the source id with the target id inside an array column. The kernel admits a declaration only when its table is one the member's own schema creates and every name it carries is a plain identifier; anything else is refused when the kernel resolves (`KernelError::Reassignment`). An account merge renders each admitted declaration into its statement from quoted identifiers, with the user ids bound, and runs all of them, in composition order, with the users-owned half of the merge in one transaction: the table a declaration names is the table its statement touches, and a failure anywhere leaves the merge undone.

### `cloud doctor` checks

`c.doctor_check(Arc::new(check))` contributes a `DoctorCheck` that `cloud doctor` runs for the checked profile's admitted composition. A check receives a `DoctorContext`: whether a named secret is present (never its value) and `DoctorProbes`, the read-only facts the doctor answers itself with a fixed read in a read-only database session (today the registered Git sources that name a credential label). A check receives no database pool, connection or handle map; a fact a new check needs is added as a probe method in `systemprompt-provider-contracts`. A probe whose database is unreachable answers `DoctorProbeError::Unreachable`, which a check reports as a warning. The checks of core and install members all run, against the same context, so an install's check sees nothing a core check does not. It returns a `DoctorOutcome` the doctor reports under the check's name.

### Configuration

`c.config::<C>()` declares a typed configuration section: `C` implements `ExtensionConfig` (`serde::Deserialize` + `schemars::JsonSchema`, a `PREFIX` and `validate`). Startup validation reads `<services>/config/<PREFIX>.yaml` (an absent file validates as `{}`) and reports a malformed section under `<PREFIX>.config`. `c.config_plane(plane)` contributes a config plane reconciled against the services tree.

### Shared state

`c.provide::<T, _>(|r| …)` provides a value of type `T` built from the resolved state; `c.requires::<T>()` declares a type the member cannot run without; `c.depends_on::<E>()` orders the member after `E`, a member of its own standing: an install member naming a core member is refused at admission (`CompositionError::CrossCapabilityDependency`), and needs no ordering, because every core member resolves first. A provide fn reads dependencies with `r.get::<T>()` (a missing one fails boot) or `r.optional::<T>()`. Each type has exactly one provider: two members providing one type are refused (`DuplicateProvider`), and a type the kernel inserts (the audience resolvers, subject provider set, policy catalog and gateway guards) cannot be provided or slot-built by any member (`KernelError::ReservedType`). An install-provided value cannot meet a core member's requirement (`InstallSubstitution`). `c.provide_after_governance::<T, _>(|r| …)` provides the same way after every slot, the subject provider set, the policy catalog and the gateway guards are built, for a value assembled from them; no slot build or governance factory can read it.

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
- a second provider of a type, a type the kernel inserts, or a value substituting for a core requirement;
- an audience resolver for a reserved subject or dimension, or a subject provider declaring `user`, `role` or `traffic`;
- a slot entry that replaces a core-only seam.

The schema installer separately refuses, before any DDL, a pending migration of the kinds listed under the schema rules above.

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
- its recorded tables are kept: the schema installer does not drop them, no other member can claim them, and the installer refuses another member's pending migration that re-creates or changes them with a judged statement kind. A member disabled before its first recorded install has no record until it is next enabled;
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

Core owns these contracts so a member never reimplements them against core tables. `ctx` below is the job's `JobContext`; each seam is a kernel seed it resolves by type.

- **Governance:** build a `PolicyRequest` and call `GovernanceEngine::charge_and_evaluate`. The engine charges every armed rate window before the chain runs.
- **Replica liveness:** `ctx.get::<DynInstanceRegistry>()?.is_alive(&instance_id)` is true while that replica holds its instance claim or heartbeats its `services` rows within 90 seconds. `ctx.get::<InstanceId>()?` is this node's identity.
- **Leases:** `ctx.get::<DynLeaseRepository>()?.claim(&LeaseKey::new("…"), ttl)` grants an expiring single-owner claim on a named resource, or `None` while another live replica holds it. A dead owner's lease is forfeit even before it expires.
- **Advisory locks:** `ctx.get::<DynAdvisoryLocks>()?.try_acquire(&AdvisoryLockKey::new("…"))` (or `acquire_within(key, wait)`) takes a cluster-wide lock on the session pool. Call `release().await` on the guard; dropping it closes the locking session, which also frees the lock.
- **Job database budgets:** override `Job::db_budget` to return a `DbBudget`, then open the job's transactions with `ctx.get::<BudgetedDb>()?.begin()`. The budget is applied with `SET LOCAL`.
- **Batched jobs:** drain a backlog with `run_batched(BatchPlan { deadline, full_batch }, || batch())`; `BatchedRun::into_job_result` reports what remains for the next run.
- **Once per release:** `ReleaseStampRepository::run_once(ReleaseRun { name, release: env!("CARGO_PKG_VERSION"), gate, wait }, work)` runs `work` under a cluster-wide lock when this release is ahead of the recorded one (`ReleaseGate::IfBehind`) or not behind it (`ReleaseGate::UnlessNewer`), and records the release only when `work` succeeds.
- **Secrets:** a member declares each custom secret it reads with `c.secret(SecretName::new("my_api_key"))` (or a prefix with `c.secret_prefix`), the profile grants it under `extensions.secrets.<member id>`, and a provide fn, factory or slot build reads it with `r.secrets()?.secret("my_api_key")` (a `DeclaredSecrets` read that returns an `ExtensionSecret`, which redacts itself in `Debug`) and provides the member's own typed client or credential, which its jobs then resolve. The secret store (`Arc<dyn SecretStore>`) is a sealed seed outside `reserved::INSTALL_SEEDS`: an install member's `r.get::<Arc<dyn SecretStore>>()` fails as a missing dependency and refuses the boot. A name a core member, the profile or the platform references cannot be declared. Do not read credentials or `HOSTNAME` from the environment.
- **Install views:** an install member's provide fn, factory or slot build reads exactly four host-sealed seeds, the rows of `reserved::INSTALL_SEEDS`: `SystemAdmin`, `Arc<ConfigView>` (the runtime configuration without `database_url`, `database_write_url`, `database_session_url` and `signing_key_path`), `Arc<ProfileView>` (profile name, target, runtime environment, `judge_automatic`, cloud tenant id, the OTLP target without its headers or URL credentials, and `paths.bin`) and `InstallServicesView` (the active services root, the services config and catalog, and the bundle source list as name, transport and credential-free location). `Arc<Config>` is not install-readable. No route set's state reads any of them: read them in a provide fn and hand the route set what it needs.
- **Conferred capabilities:** an install member that needs a sealed key uses a handle bound to itself, never the key. Declare the row in `contribute` and read the handle in a provide fn, factory or slot build: `c.conferral(InstallConferral::MemberCipher)` then `r.member_cipher()?` (authenticated encryption under a key derived for this member; another member's ciphertext does not open), `c.conferral(InstallConferral::ShareTokens)` then `r.share_tokens()?` (sign and verify this member's purpose-bound share tokens; no core verifier accepts them), `c.conferral(InstallConferral::GitSourceSync)` then `r.git_source_sync()?` in a `provide_after_governance` (sync a registered Git source with the credential core holds; the credential is never returned), and `c.token_audience("<member-id>:<name>", &["<scope>"])` then `r.token_minter()?` (mint RS256 tokens for that audience and those scopes only, at most one hour, each mint audited). Admission refuses a core audience, an audience outside the member's namespace or one the runtime configuration accepts, and a scope that is a core permission (`admin` among them). Each read returns `ConferralError`. An undeclared row, or an API whose secrets lack the key, is `ConferralError::Missing`, naming the handle type, and refuses the boot. A child host (`admin mcp serve --server <id>`, `admin agent-host run`) confers nothing, whatever it was granted: a declared read there is `ConferralError::Withheld`, which `?` in a provide fn turns into a withheld value, so the child boots without it. A provide fn that `r.get`s a withheld value is withheld in turn (its `?` yields `ProvideError::Withheld`), transitively, so a value built on a conferral never fails the child as a missing dependency; a dependency nothing provides is still `ProvideError::Missing` and refuses the boot on every host. A value an MCP toolset needs on the child is built without the handle by matching `ConferralError::Withheld`; a factory's `?` fails the slot build instead.
- **Profile grants:** every profile that boots an install member declaring secrets lists them under `extensions.secrets.<member id>`; a declared secret no grant covers refuses boot (`KernelError::UngrantedSecret`), on every profile, local ones included.
- **Scheduler config:** a `scheduler.jobs` or `scheduler.bootstrap_jobs` entry naming a job no admitted member contributes (a removed job, or one of a disabled member) refuses boot; remove the entry with the job.
- **Readiness:** a check contributed with `c.readiness` runs on every `/readyz`; a check that fails, errors or overruns the probe timeout answers 503 with the failing check under `extensions`.
- **Metrics:** emit through the `metrics` facade and declare each metric with `c.metric(MetricDescriptor)`, so a histogram renders as buckets and its help text appears on `/metrics`.

## Install seams (0.88)

Each seam below is what an install member reads in place of a sealed core handle.

- **Re-sealing pre-0.88 ciphertext.** No value is sealed under `encryption_master_key` itself. Every at-rest value, core's included, is sealed under its owning member's cipher (HMAC-derived per member id, the `spmc1.` envelope). A runtime reader that meets a value without the envelope refuses it with `AtRestCipherError::Unresealed`, which names the command below. A member, core or install, declares each column that may still hold pre-0.88 ciphertext with `c.reseal_column(ResealColumn { table, keys, ciphertext, layout })`. The layout is `SealedLayout::SplitBytea { nonce }`, with the 12-byte nonce and the ciphertext in `BYTEA` columns, or `SealedLayout::HexText`, with the hex nonce and ciphertext in one `TEXT` column; the `ai` member declares `user_provider_credentials` this way. Admission (`AdmittedReseals`) refuses a table the member's own schema SQL does not create, or that another member also creates. At run time the command also refuses a table that `schema_table_owners` does not record to the member. `systemprompt admin secrets reseal` with no `--member` reports every declaring member and its counts and writes nothing. `--member <id>` reports that member's columns: the rows in total, the rows already re-sealed, the rows it would re-seal and the undecodable rows. Add `--apply` to write: the command writes one `governance_decisions` row (policy `member_reseal`) and then rewrites rows in batches of 500, one transaction per batch, each row only while it still holds the value that was read. A value already enveloped is skipped, so a second `--apply` changes nothing. A declared key column may be nullable: a row whose key has a NULL column is named by the same compare-and-swap (Postgres array equality treats two NULLs as equal) and is re-sealed like any other. A value that opens under neither scheme and a row that changed while it was re-sealed (`unwritten`; re-run) are left as they are and make the command exit non-zero. A second decision row records the completed counts after the writes. `--member gateway` also re-keys the gateway journal's pre-0.88 segments. Every frame of a segment is probed, so one damaged frame never hides the pre-0.88 frames after it, and the journal refuses to open (`GatewayAuditError::PreUpgradeJournal`, from `GatewayJournal::open` itself) until they are re-keyed. A staged `.reseal` file left by a run that died is replaced on the next run. A value core sealed after the upgrade never opens under the old scheme, so planting it in an install's table re-seals nothing. The command refuses an implicitly selected cloud profile.

  ```rust
  c.conferral(InstallConferral::MemberCipher)
      .reseal_column(ResealColumn {
          table: "acme_credentials", keys: &["user_id", "provider"],
          ciphertext: "ciphertext", nonce: "nonce",
      });
  // then: systemprompt admin secrets reseal            # every declaring member, report only
  //       systemprompt admin secrets reseal --member acme --apply
  ```
- **Member cipher values.** `MemberCipher::seal(&[u8]) -> String` returns `spmc1.` followed by the hex nonce and ciphertext. `open(&str) -> Vec<u8>` refuses a value without that envelope, so binary plaintext such as a data key needs no extra encoding.
- **Inventory refresh.** `InventoryService::refresh(&owner, &view)` takes the `InstallServicesView` an install reads in a provide fn: the active root and its provenance, the base tree, the bundle cache and the sources without credentials. `SourcesContext::of(&view)` and `BaselineScope { owner, actor, view }` are built from the same view. The repository is the open `Arc<ManagedRepository>`.

  ```rust
  c.provide_after_governance(|r: &Resolved<'_>| Ok(Arc::new(MyInventory {
      service: InventoryService::new(r.get::<Arc<ManagedRepository>>()?.as_ref().clone()),
      view: r.get::<InstallServicesView>()?,
      owner: r.get::<SystemAdmin>()?.id().clone(),
  })));
  // in the job: inventory.service.refresh(&inventory.owner, &inventory.view).await?
  ```
- **A service built from member state.** `RouteSetBuilder::service_with(prefix, |state: &S| service, spec)` builds the nested service from the route set's own state `S` when the API materialises the set. `SiteFallback::with_state(|state: S| service)` does the same for the composition's one site fallback, so an install's main web app can be the fallback. `S` resolves through the member-scoped route resolution, so it reads what the member provided and never a host-sealed seed or an install view (Rule C). A second fallback claimant is still `CompositionError::SecondSiteFallback`. Static web content answers before the fallback: a `GET` or `HEAD` the web dist serves (pages, assets, `sitemap.xml`, feeds, `llms.txt`, `robots.txt`, and `/` when the dist holds `index.html`) is served as without a fallback, and the fallback answers what static content misses and every other method, behind site auth when a member declares it, and never a reserved path.

  ```rust
  c.routes(RouteSet::at::<WebState>(base, RouteGroup::Extensions)
      .service_with("/app", |s: &WebState| s.app.clone(), spec)
      .build());
  c.site_fallback(SiteFallback::with_state(|s: WebState| s.app));
  ```
- **Content factories may decline.** Every `ContentSlot` factory returns `Result<Option<Arc<dyn T>>, FactoryError>`. `Ok(None)` contributes nothing, and the hook is absent from `ContentHooks`, never replaced by a default. `Err` still refuses the slot.
- **Resource reads and listings carry the caller.** `McpToolset::read_resource(ResourceRead { request, caller })` and `McpToolset::list_resources(ResourceList { caller })` are async and receive the same hop-verified `VerifiedCaller` a tool call receives. The host refuses a read or a listing with no verified caller before any toolset is asked. List only what `read_resource` would serve that caller.
- **Renderers are scoped to the processes that render.** `c.slot::<UiSlot>().renderer(RenderOn::Api | RenderOn::McpServers | RenderOn::ApiAndMcpServers, factory)`. Every kernel is seeded with its `HostRole` (`Api`, `McpServer` or `Agent`). A renderer factory runs, and replaces a built-in, only on the roles its `RenderOn` covers. An agent child builds none.
- **Scoped tokens.** A token from `ScopedTokenMinter` carries only `iss`, `sub`, `aud`, `scope`, `azp`, `jti`, `iat` and `exp`. It has no session, device or client-session claim, and nothing in the request can add one. The subject is any user the member names except the system admin; the minter refuses that subject. The audience is the member's own declared audience, and no core verifier accepts the token.

## Handles an install does not get, and what to use instead

An install is granted a sealed core handle only if it already exists as a narrow, read-only trait. Every other handle stays sealed.

| Handle | Asked for by | Why it stays sealed | Use instead |
|---|---|---|---|
| `TokenAuthority` / `SessionCreationService` (browser session mint, ADFS, dev login) | template, astound, dynapps | Mints core-audience session tokens | Contribute an `SsoProvider` to `SsoSlot`; core mints the session |
| A per-server identity-token signer (external MCP identity tokens with email and name claims) | template, astound, dynapps | No narrow trait exists; it would sign operator-chosen audiences with user claims | `ScopedTokenMinter` for a `<member>:` audience; identity-backed servers stay core's |
| Core-audience tokens for workflow runners (`mcp` audience) | astound | Core audience | Core's own runner path |
| `mcp_credential_broker_secret` | template, dynapps | A platform secret | Not readable; the broker is core's |
| A keyed HMAC over the master key (login tracker) | astound | Would export a deterministic key oracle | Generate a key and keep it sealed with `MemberCipher` in your own table |
| `DynAiProvider` / `AiService` | template, dynapps | Spends credentials and quota on the caller's behalf; not read-only | None in 0.88; the consuming job stays idle |
| `AiRepositories` (personal provider credentials) | dynapps | Holds credential rows | None in 0.88 |
| `DocumentsHandles` / `DocumentService` | dynapps | Read and write across every owner | None in 0.88 |
| `SharedAuthzHook` | dynapps | Evaluating writes decision rows; it is the core decision point | Contribute an authz hook to `AuthzHookSlot`; core composes it |
| `ConfigPlaneSet` | dynapps | Applies core config planes | `admin config apply` |
| `InventoryPublisher` / `InventoryRefresher` | template, dynapps | Publishes under the system admin | `InventoryService::refresh` over `InstallServicesView`; core's `inventory_refresh` job publishes |
| `ProviderTokenCache` | dynapps | Caches connector bearers of every user | None in 0.88 |
| Secret key enumeration, status and reload | dynapps | Lists core secret names | Declare and read your own names with `c.secret` |
| `Arc<dyn SecretPresence>` (is a secret set?) | dynapps | Answers presence for every secret name; provided sealed, conferred only on `agent-http`'s commands | Declare the secret and read it with `r.secrets()`; absence is `None` |
| `encryption_master_key` granted to a spawned server or agent | dynapps, astound | The raw key derives every member's key; the grant is refused at config validation | `MemberCipher` (conferred), and `admin secrets reseal` for pre-0.88 values |
| `Arc<Config>`, `SecretStore`, signing keys | all | Host-sealed | `ConfigView`, declared secrets, the conferred capabilities |
| An in-process API host exposing the token authority and the documents service | astound | Hands out core minting and cross-owner document access | Drive the HTTP surface of a composed test server |
| Overriding the external URL on `ConfigView` | astound | The view is read-only by design; the URL is what core's verifiers pin | Set `server.external_url` in the test profile |

### Testing an MCP toolset with a verified caller

A `VerifiedCaller` is the hop token the host admitted plus the context it built from it. It is evidence, not an assertion: `VerifiedCaller::presented(hop, context)` is public, and core's tool execution decodes the hop token again for its server and refuses, recording nothing, a caller whose token does not verify or does not vouch for the context. A test of a toolset's dispatch builds a real one with `systemprompt_test_fixtures::fixture_verified_caller(&server, &ctx)` (a hop token minted under the fixture signing keys for that server, the context bound to it) and an executor with `fixture_caller_verification()`. A test of the full host path follows it: it mints a hop token with `TokenAuthority::mint_mcp_hop(&server, &identity)` under the test signing keys, then calls `systemprompt_mcp_host::serving::middleware::admit_request(&HopVerifier::new(keyring, external_url, server), &headers, &mut parts.extensions)`, which inserts the `VerifiedCaller`. It then serves the `ToolsetServer` over an in-memory transport, as `crates/tests/unit/domain/mcp/src/toolset_server_gate.rs` does. A caller built from a forged, expired or other-server token, or a context for a different user than its token, is refused.

## Upgrading from 0.82

- Move each defaulted hook body (`schemas`, `migrations`, `routers`, `jobs`, `provide`, `requires`, the seam hooks) into `contribute`, and replace each `register_*!`/`submit_*!` registration with the matching `Contributions` method or the owning crate's `contract` slot. Drop the `inventory` dependency.
- Replace every `ExtensionContext` read with the typed seed: `ctx.get::<Arc<dyn FileStorage>>()`, `ctx.get::<InstanceId>()`, `ctx.get::<DynLeaseRepository>()` and so on in a job; `r.get::<T>()` in a provide fn, factory or route set state. The install views (`Arc<ConfigView>`, `Arc<ProfileView>`, `InstallServicesView`), the system admin, declared secrets (`r.secrets()`) and the conferred capabilities are read in a provide fn only; those are the only sealed values an install member's provide fn reads (`reserved::INSTALL_SEEDS` lists `SystemAdmin`, `ConfigView`, `ProfileView` and `ServicesView`). Replace an `Arc<Config>` read with `Arc<ConfigView>`.
- Replace `ExtensionRouter` with `RouteSet`s under the member's own `/api/` base or the web root, and any root fallback with an explicit prefix.
- Move command groups to `run(&dyn CommandHost) -> CommandOutput`.
- Replace the binary's `main` with `reference().with(..).run()`.
- Run one install with every member enabled, so table ownership and purge declarations are recorded for each member. When a core release newly creates a table or object that a disabled or held-back install member has recorded, every full install is refused until that member is enabled for one install; that install moves the record to core. A profile with disabled members runs one full schema install on its next boot.

## Verify

```bash
cargo check -p <your-extension-crate>
systemprompt plugins list
systemprompt plugins seams --member <id>
```

`plugins list` and `plugins seams` report the admitted composition member by member. Compilation alone does not admit a member: run the binary against a profile to see admission accept it.

An install's tests install its schema on a fresh database the way `infra db migrate` does: admit the composition against a test profile and hand the admitted members to the schema installer.

```rust
let admitted = reference().with(DemoExtension).build(&profile)?;
let database = Database::new_postgres(&database_url).await?;
install_admitted_schemas(
    admitted.built(),
    database.session(),
    &SchemaName::public(),
    MigrationConfig::default(),
)
.await?;
```

`install_admitted_schemas`, `Database`, `SchemaName` and `MigrationConfig` are in `systemprompt::database`; `build` refuses a composition admission would refuse, so the test database holds exactly what the binary installs.

## Related pages

- [concepts/extensions.md](../concepts/extensions.md) — the composition contract and how the pieces fit together.
- [configure-providers.md](configure-providers.md) — configuring AI providers through the gateway.
