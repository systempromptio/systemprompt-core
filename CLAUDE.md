# systemprompt.io Core

The core platform engine for systemprompt.io - a multi-tenant AI agent platform with A2A protocol support, MCP integration, and cloud deployment.

## Architecture

```
systemprompt-core/
├── crates/
│   ├── shared/           # Foundation layer (no dependencies on other layers)
│   │   ├── models/       # systemprompt-models - Core data types
│   │   ├── traits/       # systemprompt-traits - Core interfaces
│   │   ├── identifiers/  # systemprompt-identifiers - Typed IDs
│   │   ├── extension/    # systemprompt-extension - Extension framework
│   │   ├── provider-contracts/  # Provider trait definitions
│   │   ├── client/       # HTTP API client
│   │   └── template-provider/   # Template traits
│   │
│   ├── infra/            # Infrastructure layer
│   │   ├── database/     # systemprompt-database - SQLx abstraction
│   │   ├── events/       # systemprompt-events - Event bus, SSE
│   │   ├── security/     # systemprompt-security - JWT, auth
│   │   ├── config/       # systemprompt-config - Config loading
│   │   ├── logging/      # systemprompt-logging - Tracing setup
│   │   ├── loader/       # systemprompt-loader - File/module discovery
│   │   └── cloud/        # systemprompt-cloud - Cloud API, tenants
│   │
│   ├── domain/           # Business logic layer
│   │   ├── users/        # systemprompt-users - User management
│   │   ├── oauth/        # systemprompt-oauth - OAuth2/OIDC
│   │   ├── files/        # systemprompt-files - File storage
│   │   ├── analytics/    # systemprompt-analytics - Metrics
│   │   ├── content/      # systemprompt-content - Content management
│   │   ├── ai/           # systemprompt-ai - LLM integration
│   │   ├── mcp/          # systemprompt-mcp - MCP protocol
│   │   ├── agent/        # systemprompt-agent - A2A protocol
│   │   └── templates/    # systemprompt-templates - Template registry
│   │
│   ├── app/              # Application services layer
│   │   ├── runtime/      # systemprompt-runtime - AppContext, lifecycle
│   │   ├── scheduler/    # systemprompt-scheduler - Job scheduling
│   │   ├── generator/    # systemprompt-generator - Static site gen
│   │   └── sync/         # systemprompt-sync - Cloud sync
│   │
│   ├── entry/            # Application boundaries
│   │   ├── api/          # systemprompt-api - HTTP server
│   │   └── cli/          # systemprompt-cli - CLI application
│   │
│   └── tests/            # Separate test workspace (excluded from main)
│
├── systemprompt/         # Facade crate - re-exports with feature flags
├── defaults/             # Default templates, assets, web content
├── documentation/        # External evaluation pack (committed, RFI/procurement-safe)
└── internal/             # Local-only dev docs (gitignored)
    ├── guides/          # Architecture, boundaries, standards, release, runbooks
    ├── audits/          # Crate-by-crate compliance audits
    ├── reports/         # Codebase evaluations, test roadmaps
    └── legal/           # License compliance / SBOM
```

## Dependency Flow

```
Entry (api, cli) → App (runtime, scheduler) → Domain (agent, ai, mcp...) → Infra (database, events...) → Shared (models, traits)
```

**Rule**: Dependencies flow downward only. No circular dependencies.

## Documentation Layout

Two buckets, kept strictly separate:

- **`documentation/`** — the **external** evaluation pack: neutral, stable, committed, and safe to cite in an RFI / procurement / security review. No internal repo names, CI secrets, version-drift snapshots, or work-plans. Flat layout (security/RFI material at the top level). Anything here must read cleanly for an external consumer.
- **`internal/`** — **local-only** engineering docs, **gitignored** (a few force-added files aside). Organised by purpose: `guides/` (architecture, boundaries, standards, release process, bridge build/release, runbooks), `audits/`, `reports/` (codebase evaluations, test roadmaps, deployment-scenario work), `legal/` (license/SBOM).

When adding docs: external-consumer material → `documentation/`; anything about *how we build, release, audit, or evaluate* → `internal/` (use `git add -f` only if a specific internal file must be shared). Never deep-link from a committed file into `internal/` — external readers cannot see it.

## Repository Hygiene

This is a world-class, public, code-only repository. **Only code and proper
public-facing files belong in git**: source, `Cargo.toml`/`build.rs`,
`README.md`, `CHANGELOG.md`, schema/migration `*.sql`, and legitimate test
fixtures (proptest/fuzz seeds, protocol specs). Everything else stays out.

**Never committed** (these are local-only dev artefacts): status reports,
test-planning stubs, coverage trackers, implementation guides, scratch notes, or
any process doc (`*STATUS*`, `*PLAN*`, `*REPORT*`, `*SUMMARY*`, `*GUIDE*`,
`*PROGRESS*`, `*FINDINGS*`), and build/tooling output. The `ci/` scripts and
`internal/` tree are local-only and `.gitignore`d.

**No new folders — or process docs — are added to git without explicit user
approval.** Before any commit, sweep the staged tree
(`git ls-files | grep -iE '(status|plan|report|summary|guide|progress|findings)'`
and `git ls-files 'crates/**/*.md' | grep -vE '/(README|CHANGELOG)\.md$'`) and
confirm nothing stray slipped in. Never `git add` a directory you did not create
as part of an approved change.

## Key Documentation

| Document | Purpose |
|----------|---------|
| `internal/guides/architecture.md` | Full crate taxonomy, extension framework, paths |
| `internal/guides/boundaries.md` | Module boundary rules, acceptable patterns |
| `internal/guides/cloud.md` | Cloud deployment and tenant management |
| `internal/guides/rust.md` | Rust coding standards (mirrors the `rust-coding-standards` skill) |
| `internal/guides/bridge/` | Bridge build, release, versioning, per-OS reference |
| `documentation/` | External evaluation pack (security, compliance, stability, RFI) |

## Rust Standards

**MANDATORY**: the marketplace skill `rust-coding-standards` is the canonical source of truth. `internal/guides/rust.md` and this file mirror it — when they diverge, the skill wins. Key rules:

- **Inline `//` comments**: banned for WHAT-comments. Permitted ONLY when encoding a non-obvious *why* (hidden constraint, subtle invariant, bug-workaround). Never narrate "what we just changed" or reference past callers/issues.
- **`///` rustdoc**: one uniform rule across **all** production crates incl. `entry/*` (entry is not special). NOT applied mechanically per pub item. Real `//!` blocks live on `lib.rs` and significant `pub mod` files (purpose, public surface, feature matrix, error model) everywhere. Per-item `///` is added only where it captures non-obvious value — paraphrasing the function name and signature is a code smell. `///` is banned only inside `crates/tests/**`.
- **Typed identifiers**: zero raw String IDs in struct fields or service args — use wrappers from `systemprompt_identifiers`. Construct via `Id::new(s)`, `Id::try_new(s)?`, or `Id::generate()`. Never `.into()` or `::from()` at call sites.
- **Repository pattern**: services never run SQL directly. All queries via compile-time verified macros (`sqlx::query!()`, `sqlx::query_as!()`, `sqlx::query_scalar!()`). Runtime `sqlx::query(_)` is permitted ONLY in `crates/infra/database/src/admin/**`, `crates/infra/database/src/services/postgres/{introspection,query_executor,transaction,ext,mod}.rs`, and `crates/entry/cli/src/commands/admin/setup/**` (bootstrap DDL — `CREATE USER` / `CREATE DATABASE` / `GRANT` run before the target database exists; DDL cannot bind parameters) where dynamic SQL is the contract.
- **Errors**: `thiserror`-derived enums in published library crates (`shared/*`, `infra/*`, `domain/*`, `app/*`, `systemprompt` facade). `anyhow::Error` is forbidden in public signatures of library crates; permitted only in `entry/cli`, `entry/api`, `build.rs`, and tests.
- **Async traits**: native `async fn` by default. `#[async_trait]` only when the trait must be `dyn`-compatible — document the reason on the trait.
- **Logging**: all logging via `tracing` with structured fields. `println!` / `eprintln!` / `dbg!` banned in libraries; carve-outs are the CLI display sinks in `crates/infra/logging/services/cli/**` and `crates/infra/database/src/services/display.rs`, plus `cargo:rerun-if-changed=` directives in build scripts.
- **No legacy code, backwards-compat shims, dual code paths, or `Option<T>` migration stubs**: land the new code AND delete the old form in the same PR.
- **Naming**: `*Service` by default, `*Handler` only for HTTP/RPC handlers, `*Orchestrator` for cross-domain workflows. Avoid `*Manager`.
- **Schema DDL & migrations**: schema DDL lives in `{crate}/schema/*.sql`, embedded via `include_str!()` in `extension.rs`. Migration SQL lives in `{crate}/schema/migrations/NNN_<name>.sql`, discovered by the crate's `build.rs` (`systemprompt_extension::build::emit_migrations()`) and returned via the `extension_migrations!()` macro — never inline SQL string constants or a hand-written `Migration::new(...)` list. Pre-merge: `just lint-extensions`.

Run after changes: `cargo +nightly fmt --all && cargo clippy --workspace --all-targets --all-features -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features && just file-size`.

### Typed Identifiers

Canonical construction forms for IDs declared via `define_id!`:

```rust
// 1. Known string value (literal, parsed input, DB row, function arg)
let id = AgentId::new("agent_one");
let id = AgentId::new(s);

// 2. Mint a fresh UUID (only for IDs declared with the `generate` flag)
let id = SessionId::generate();

// 3. Validated / non_empty IDs in fallible contexts
let id = TenantId::try_new(s)?;
```

Disallowed for typed IDs in normal code (these compile but hide the type at the call site):

```rust
let id: AgentId = "agent_one".into();   // ❌
let id = AgentId::from("agent_one");    // ❌
```

The macro-generated `From` / `TryFrom` impls remain on the type — they are required for generic `Into<T>` bounds and for serde. The rule is about call-site idiom only.

This is a convention, not a hard-enforced lint — a clippy/dylint rule was evaluated and rejected as too brittle. Reviewers should call out violations.

## Facade Crate (`systemprompt/`)

Re-exports all functionality with feature flags:

| Feature | Includes |
|---------|----------|
| `core` (default) | traits, models, identifiers, extension |
| `database` | Database abstraction |
| `api` | HTTP server, AppContext (requires core + database) |
| `cli` | CLI entry point |
| `full` | Everything: all domain modules + CLI |

```rust
// Using the facade
use systemprompt::prelude::*;
use systemprompt::database::DbPool;
```

## Extension Framework

Extensions use the `inventory` crate for compile-time registration:

```rust
use systemprompt::extension::prelude::*;

struct MyExtension;
impl Extension for MyExtension {
    fn metadata(&self) -> ExtensionMetadata { ... }
    fn schemas(&self) -> Vec<SchemaDefinition> { ... }
    fn router(&self) -> Option<ExtensionRouter> { ... }
}

register_extension!(MyExtension);
```

**Key traits**: `Extension`, `SchemaExtensionTyped`, `ApiExtensionTyped`, `JobExtensionTyped`, `ProviderExtensionTyped`

## Configuration

Profiles are the primary source of truth. Environment variables are a scoped
escape hatch, not a general fallback: profile YAML may interpolate `${VAR}`, and
a small set of sanctioned overrides exists for cloud/subprocess boots
(`SYSTEMPROMPT_SYSTEM_ADMIN`, `SYSTEMPROMPT_SERVICES_PATH`,
`SYSTEMPROMPT_SKILLS_PATH`, `SYSTEMPROMPT_CONFIG_PATH`, the secrets `env` source,
and the Fly/subprocess secret-injection path). Outside these, there are no env
fallbacks — config comes from the profile:

```yaml
# .systemprompt/profiles/local/profile.yaml
name: local
database:
  type: postgres
  url: postgresql://user:pass@localhost:5432/db
server:
  host: 127.0.0.1
  port: 8080
paths:
  system: /var/www/html/myapp
  services: /var/www/html/myapp/services
secrets:
  secrets_path: ../secrets/local.secrets.json
```

**Bootstrap sequence**: ProfileBootstrap → SecretsBootstrap → CredentialsBootstrap → Config → AppContext

## CLI Commands

```bash
# Services
systemprompt infra services start --all
systemprompt infra services status
systemprompt infra services stop --all

# Database
systemprompt infra db status
systemprompt infra db migrate
systemprompt infra db query "SELECT * FROM users LIMIT 10"

# Agents
systemprompt admin agents list
systemprompt admin agents status my-agent

# Cloud
systemprompt cloud auth login
systemprompt cloud tenant create
systemprompt cloud deploy
```

## Database Pattern

```rust
use systemprompt_database::DbPool;

pub struct UserRepository {
    pool: DbPool,
}

impl UserRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub async fn find_by_id(&self, id: &UserId) -> Result<Option<User>> {
        sqlx::query_as!(User, "SELECT * FROM users WHERE id = $1", id.as_str())
            .fetch_optional(self.pool.as_ref())
            .await
            .map_err(Into::into)
    }
}
```

## A2A Protocol

Core types in `crates/domain/agent/`:
- **Message**: `role`, `parts`, `messageId`, `contextId`
- **Task**: `id`, `contextId`, `status`, `history`, `artifacts`
- **TaskState**: Pending → Submitted → Working → Completed/Failed/Canceled

## MCP Protocol

MCP implementation in `crates/domain/mcp/`:
- Server lifecycle management
- Tool/resource discovery
- Transport protocols (stdio, SSE)

## Key Files

| File | Purpose |
|------|---------|
| `crates/entry/cli/src/main.rs` | CLI entry point |
| `crates/entry/api/src/main.rs` | API server entry |
| `crates/app/runtime/src/context.rs` | AppContext definition |
| `crates/shared/models/src/config.rs` | Config struct |
| `crates/shared/extension/src/lib.rs` | Extension trait |

## Testing

Tests are in a separate workspace at `crates/tests/` (66 crates, ~12k tests),
excluded from the main workspace. The suite is too large to compile in one pass
(`cargo test --workspace` links all 66 test binaries at once and OOMs the host),
so it runs **sharded** under `cargo-nextest` — the same 7-shard split CI uses.
The shard definitions live in `scripts/test-shard.sh`, the single source of
truth shared by CI and the `just` recipes below.

Each shard runs against a **fresh, freshly-migrated database** — never the dev
`systemprompt-web` DB (its web-project triggers break core tests). The recipes
drop+recreate the target DB (override with `TEST_DATABASE_URL`; default is a
disposable `systemprompt_test`).

```bash
# One-time: install the prebuilt nextest binary (no compile)
just install-nextest

# Run one shard (bounded compile + run memory, fresh migrated DB).
# Groups: shared infra domain app-entry bridge integration edge
just test-shard shared

# Run all 7 shards sequentially (each against its own fresh DB)
just test-all-shards

# Iterate on a single crate
just unit-test-crate systemprompt-agent-tests
```

The OOM-prone `just unit-test` / `just test-rust --workspace` forms still exist
but are not the recommended path.

## Building

```bash
# Debug build
cargo build --workspace

# Release build
cargo build --release --workspace

# Specific crate
cargo build -p systemprompt-cli
```
