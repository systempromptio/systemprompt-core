# systemprompt.io Core

Core platform engine for systemprompt.io — a multi-tenant AI agent platform with A2A protocol support, MCP integration, and cloud deployment.

## Architecture

Five layers under `crates/`; dependencies flow downward only, no cycles:

```
entry (api, cli) → app (runtime, scheduler, generator) → domain → infra → shared
```

- **shared** — models, traits, identifiers (typed IDs), extension framework, provider-contracts, client, template-provider
- **infra** — database (SQLx), events, security (JWT/authz), config, logging, loader, cloud
- **domain** — users, oauth, files, analytics, content, ai, mcp, agent, templates, marketplace, slack, teams, evaluation. Domain crates are **peers**: no domain→domain deps (see Rust Standards)
- **app** — runtime (`AppContext`), scheduler, generator
- **entry** — api (HTTP server), cli
- `systemprompt/` — facade crate re-exporting everything behind feature flags (`core`, `database`, `api`, `cli`, `full`)
- `crates/tests/` — separate test workspace, excluded from the main workspace

## Documentation Layout

- **`documentation/`** — external evaluation pack: committed, neutral, safe to cite in an RFI/security review. Nothing internal (repo names, CI secrets, work-plans).
- **`internal/`** — local-only engineering docs, gitignored (`guides/`, `audits/`, `reports/`, `legal/`). Never deep-link into `internal/` from a committed file.

New docs: external-consumer material → `documentation/`; anything about how we build/release/audit → `internal/`.

Key guides: `internal/guides/architecture.md` (crate taxonomy), `boundaries.md` (module boundaries), `cloud.md`, `rust.md` (mirrors the `rust-coding-standards` skill), `bridge/`.

## Repository Hygiene

Public, code-only repository. In git: source, `Cargo.toml`/`build.rs`, `README.md`, `CHANGELOG.md`, schema/migration `*.sql`, legitimate test fixtures. **Never committed**: status/plan/report/summary/guide/progress/findings docs, coverage trackers, scratch notes, build output. `ci/` and `internal/` are gitignored.

No new folders or process docs enter git without explicit user approval. Before any commit, sweep the staged tree:
`git ls-files | grep -iE '(status|plan|report|summary|guide|progress|findings)'` and `git ls-files 'crates/**/*.md' | grep -vE '/(README|CHANGELOG)\.md$'`.

## Rust Standards

**MANDATORY**: the marketplace skill `rust-coding-standards` is canonical; `internal/guides/rust.md` and this file mirror it — when they diverge, the skill wins.

- **Inline `//` comments**: banned for WHAT-comments. Permitted only for a non-obvious *why* (hidden constraint, invariant, bug-workaround), and rarely — the default is no comment.
- **`///` rustdoc**: uniform across all production crates incl. `entry/*`. `//!` blocks on `lib.rs` and significant `pub mod` files; per-item `///` only on **pub traits, top-level types, and `mod` declarations** (and only for non-obvious value) — banned on fns, methods, consts, fields, variants, and macros (gate: `scripts/lint-inline-comments.sh`). `///` is banned inside `crates/tests/**`.
- **Typed identifiers**: no raw String IDs in struct fields or service args — use `systemprompt_identifiers` wrappers. Construct via `Id::new(s)` / `Id::try_new(s)?` / `Id::generate()`; never `.into()` or `::from()` at call sites (convention, reviewer-enforced).
- **Repository pattern**: services never run SQL directly; all queries via compile-time macros (`sqlx::query!` family). Runtime `sqlx::query(_)` only in `infra/database/src/admin/**`, `infra/database/src/services/postgres/{introspection,query_executor,transaction,ext,mod}.rs`, and `entry/cli/src/commands/admin/setup/**` (bootstrap DDL).
- **Repository construction**: repositories are built once at composition roots (the `AppContext` builder, router-scoped state, or an owning service's ctor that stores them as fields) and injected. Consumers use the `AppContext` repository accessors (`a2a_repositories()`, `content_repositories()`, … — see `crates/app/runtime/src/context/mod.rs`) or the owning struct's field. Ad-hoc `Repo::new(&pool)` in handler/method bodies is gated by `just lint-repo-construction` (CLI one-shot commands and job bodies are exempt).
- **Errors**: `thiserror` enums in library crates; `anyhow` only in `entry/cli`, `entry/api`, `build.rs`, and tests.
- **Async traits**: native `async fn`; `#[async_trait]` only for `dyn`-compatibility, documented on the trait.
- **Logging**: `tracing` with structured fields. `println!`/`eprintln!`/`dbg!` banned in libraries (carve-outs: CLI display sinks in `infra/logging/services/cli/**`, `infra/database/src/services/display.rs`, `cargo:` build-script directives).
- **No domain→domain deps**: cross-domain capability flows through shared-layer traits (`DynAiProvider`, `ToolProvider`, `SessionUsageCounters`, provider-contracts) or infra, wired at app/entry composition roots. Enforced by `just lint-layers`; its allowlist is empty by design.
- **No legacy code**: no shims, dual paths, or `Option<T>` migration stubs — land the new form and delete the old in the same PR.
- **Naming**: `*Service` default, `*Handler` for HTTP/RPC handlers, `*Orchestrator` for cross-domain workflows. Avoid `*Manager`.
- **Schema DDL & migrations**: DDL in `{crate}/schema/*.sql` embedded via `include_str!()` in `extension.rs`; migrations in `{crate}/schema/migrations/NNN_<name>.sql`, discovered by `build.rs` (`systemprompt_extension::build::emit_migrations()`) and returned via `extension_migrations!()`. Never inline SQL constants or hand-written migration lists. Gate: `just lint-extensions`.

Run after changes: `cargo +nightly fmt --all && cargo clippy --workspace --all-targets --all-features -- -D warnings && just doc-check && just file-size`. `just doc-check` covers **both** workspaces — a bare `cargo doc --workspace` misses `crates/tests/`.

## Extension Framework

Extensions register at compile time via `inventory` (`register_extension!`); implement `Extension` (`metadata()`, `schemas()`, `router()`, `migrations()`). Key traits: `Extension`, `SchemaExtensionTyped`, `ApiExtensionTyped`, `JobExtensionTyped`, `ProviderExtensionTyped`.

## Configuration

Profiles (`.systemprompt/profiles/<name>/profile.yaml`) are the source of truth: database URL, server host/port, paths, secrets path. Env vars are a scoped escape hatch only — profile YAML may interpolate `${VAR}`, plus a small sanctioned set for cloud/subprocess boots (`SYSTEMPROMPT_SYSTEM_ADMIN`, `SYSTEMPROMPT_SERVICES_PATH`, `SYSTEMPROMPT_SKILLS_PATH`, `SYSTEMPROMPT_CONFIG_PATH`, the secrets `env` source, Fly secret injection). No other env fallbacks.

Bootstrap: ProfileBootstrap → SecretsBootstrap → CredentialsBootstrap → Config → AppContext.

## Building, Running & Schema Validation

```bash
cargo build --workspace        # online: sqlx macros validate against the live dev DB
just build-offline             # offline: committed .sqlx cache, no DB required
```

**Core has no runnable local profile** — nothing here to `start` or `migrate` against. Running and end-to-end validation happen in **`../systemprompt-template`** (always kept in sync with core):

1. Point the template at local core via `[patch.crates-io]` (never bump its version pins just to validate).
2. Use the template's recipes: `just build` = offline CLI build → `infra db migrate` → online sqlx-validated build; `just start` migrates before serving. Schema changes cannot deadlock or drift its DB.

**Schema-change gotcha in core**: `.cargo/config.toml` (and `crates/tests/.cargo/config.toml`) pin live `DATABASE_URL`s, so after adding a migration the dev DBs are behind the code and online `cargo check` fails at the sqlx macro. Apply the new `schema/migrations/*.sql` to those DBs (or run the template flow) first; `just check-offline` / `just build-offline` always work.

## Testing

Separate workspace at `crates/tests/`, run **sharded** under `cargo-nextest`. Shard definitions live in `scripts/test-shard.sh` (single source of truth for CI and the recipes; `scripts/test-shard.sh --list` prints the current groups). Each shard runs against a fresh, freshly-migrated database — never the `systemprompt-web` dev DB (its triggers break core tests). Override the target with `TEST_DATABASE_URL`; the default is a disposable `systemprompt_test`.

```bash
just install-nextest                        # one-time, prebuilt binary
just test-shard domain                      # one shard, fresh migrated DB
just test-all-shards                        # all shards sequentially
just unit-test-crate systemprompt-agent-tests   # iterate on one crate
just coverage                               # line-coverage summary
```

`test-shard` / `test-all-shards` are the supported path — they bound compile/run memory and match CI exactly.
