# CLI friction — core-side fixes

Findings from a CLI smoke test in the systemprompt-template repo. All items below are in core; template-side items live in `/var/www/html/systemprompt-template/CLI_FRICTION_PLAN.md`.

## Items

### C1. Bootstrap admin resolver should accept any user with `admin` role
**Severity:** medium (ad-hoc job runs broken on fresh installs without literal `name='admin'`)
**File:** `crates/entry/cli/src/commands/infrastructure/jobs/run.rs:152-169` (caller)
**File:** wherever `bootstrap_admin_owner` resolves (likely `crates/app/scheduler/` or `crates/shared/models/src/services/scheduler.rs`)

Current behavior: ad-hoc `infra jobs run <name>` requires a `users` row with `name = 'admin'`. Returns:
```
bootstrap admin owner does not resolve to a user; seed an `admin` user before running ad-hoc jobs
```

Scheduled startup runs succeed because they go through a different path that uses `UserId::admin()` as a static bootstrap value without DB lookup.

**Fix:** Change the resolver to match any user where `'admin' = ANY(roles)`. Prefer the user named `admin` if present (deterministic ordering), else any admin-role user (lowest `id` for stability). Update the error message to:
```
no user with role 'admin' exists; create one before running ad-hoc jobs (just cli admin users create --role admin ...)
```

Both startup and ad-hoc paths should call the same resolver — eliminate the two-path divergence as part of this fix.

### C2. `plugins mcp list-packages` — `Config not initialized` error
**Severity:** medium (single subcommand broken; all other `plugins mcp` leaves work)
**File:** `crates/entry/cli/src/commands/plugins/mcp/list_packages.rs:21`

Calls `RegistryManager::get_enabled_servers()` which calls `Config::get()` → returns `ConfigError::NotInitialized` from `crates/shared/models/src/errors/parse.rs:22`.

**Fix:** Initialize the config in this subcommand's setup, matching what sibling subcommands (`list`, `status`, `validate`, etc.) do. Likely a missing `Config::init_from_profile(...)` call in the handler's entry point. Look at `plugins/mcp/list.rs` for the working pattern and mirror it.

### C3. `web validate` doubled-`services/` path
**Severity:** low (misleading error string only)
**File:** `crates/entry/cli/src/commands/web/paths.rs:59-64`

Joins `services_path` (already `…/services`) with `templates_path` (already `services/web/templates`), producing `…/services/services/web/templates/…`.

**Fix:** Strip a leading `services/` from `templates_path` before the join, or change the join to use only the trailing path component. Same review for adjacent `assets_path`, `content_types_path` etc. — they likely have the same bug.

### C4. `plugins validate` warning references nonexistent command
**Severity:** trivial (one-string fix)
**File:** `crates/entry/cli/src/commands/plugins/validate.rs:62`

Current: `"Use 'systemprompt infra validate'"` — that command doesn't exist.

**Fix:** Change to `"Use 'systemprompt infra db validate'"`. Or, if the intent was to validate assets specifically (not schema), drop the suggestion entirely — the warning's value is the diagnosis, not the redirect.

### C5. Blank subcommand descriptions in `--help`
**Severity:** low (discovery friction)
**Files:**
- `crates/entry/cli/src/commands/plugins/mcp/mod.rs:24-39` — `McpCommands` enum: `List`, `Status`, `Validate`, `Logs`, `ListPackages`, `Tools`, `Call` all lack `#[command(about = "...")]`.
- `crates/entry/cli/src/commands/cloud/sync/mod.rs:14-22` — `SyncCommands::Push`, `SyncCommands::Pull` lack descriptions (`AdminUser` at line 20 has one — use as template).

**Fix:** Add `#[command(about = "…")]` (or `///` doc comments, depending on the clap derive style used elsewhere in the file) to each variant. Match length and tone to sibling enums. Sample descriptions:

```
List          → List enabled MCP servers
Status        → Show MCP server runtime status
Validate      → Validate MCP server configurations
Logs          → Tail logs for an MCP server
ListPackages  → List discovered MCP packages from the registry
Tools         → List tools exposed by enabled MCP servers
Call          → Invoke a tool on an MCP server
Push          → Push local state to cloud
Pull          → Pull cloud state to local
```

### C6. `admin access-control export-yaml` returns JSON-escaped YAML
**Severity:** low (copy-paste UX defeated)
**File:** `crates/entry/cli/src/commands/admin/access_control.rs:45-63`

Handler wraps raw YAML string in `CommandResult::copy_paste()` at line 48. The renderer for that result kind serializes via JSON (escaping `\n`), defeating the "paste this YAML" purpose.

**Fix:** Either:
- (preferred) Add a `CommandResult::raw_text(String)` variant that prints verbatim to stdout with no escaping/wrapping. Use it here.
- Or bypass the result wrapper and `println!("{}", yaml_string)` directly in this handler, accepting the inconsistency.

Audit other call sites of `copy_paste(...)` for the same problem.

### C7. INFO-level tracing leaks into one-shot CLI output
**Severity:** low (output readability)
**Files:**
- Default log level: `crates/entry/cli/src/lib.rs:60-61` (`resolve_log_level()` returns `None`)
- Emit sites:
  - `crates/shared/extension/src/registry/discovery.rs:68` — "Extension discovery completed"
  - `crates/app/runtime/src/builder.rs:189` — "marketplace filter registered via inventory"

These emit on every one-shot CLI invocation, polluting stdout/stderr.

**Fix:** Two options, take both:
1. Default log level for CLI subcommands should be `WARN`, not whatever-inherits-from-env. Set `resolve_log_level()` to default to `Level::WARN` when none of `-v`/`--debug`/`RUST_LOG` is set.
2. Demote the two specific emit sites to `DEBUG`. They're useful for `--debug` runs, not for production CLI output.

### C8. `infra db validate` confusing "Expected: 25, Actual: 147" line
**Severity:** low (misleading telemetry)
**File:** `crates/entry/cli/src/commands/infrastructure/db/schema.rs:225-226` (compute), `:260` (print)

`expected_tables` = sum of extension-declared schemas (25). `actual_tables` = `COUNT(*) FROM information_schema.tables` (147 — includes Postgres system tables and migration tracking). The comparison is meaningless; the real signal is the missing-table list further down.

**Fix:** Either:
- (preferred) Drop the "Expected/Actual" line entirely. The missing-table list IS the validation output.
- Or relabel to `"Declared by extensions: 25 | Present in database: 147 (incl. system)"` so the operator doesn't think 147 ≫ 25 is an error.

### C9. Sweep: remove other legacy / no-longer-implemented features encountered
**Severity:** varies

While in core, scan for and remove:
- Any other references to `UserId::admin()` as a literal vs the new role-based resolver (post-C1).
- Stale `system_user_id` / `is_system` columns or references (the design rejected these per the prior W-plan).
- Any clap subcommands whose handler is `todo!()` or returns `unimplemented` — either implement or remove from the enum.
- `CommandResult` variants that no renderer handles (dead code in `copy_paste` post-C6 fix could leave one).

Grep targets:
```bash
rg 'UserId::admin\(\)' --type rust
rg 'is_system|system_user_id' --type rust
rg 'todo!\(\)|unimplemented!\(\)' --type rust crates/entry/cli/
```

Make this a single sweep commit at the end, after C1–C8 land, so the diff is reviewable.

## Verification

```bash
# C1
just cli admin users delete admin   # if it exists
just cli infra jobs run content_analytics_aggregation
# Expect: success, attributed to first admin-role user.

# C2
just cli plugins mcp list-packages
# Expect: real output, not "Config not initialized".

# C3
just cli web validate
# Expect: no doubled `services/services/` in any path.

# C4
just cli plugins validate 2>&1 | grep -F "infra validate"
# Expect: no match (either gone or now says "infra db validate").

# C5
for sub in list status validate logs list-packages tools call; do
    just cli plugins mcp --help | grep -E "^\s+$sub\s+\S"
done
# Expect: every sub line has a non-blank description.
just cli cloud sync --help | grep -E "^\s+(push|pull)\s+\S"

# C6
just cli admin access-control export-yaml | head -5
# Expect: literal newlines, not "\n" escapes. Pipe to a file and it should be valid YAML.

# C7
just cli core skills list 2>&1 | grep -E "Extension discovery completed|marketplace filter registered"
# Expect: no matches at default verbosity.

# C8
just cli infra db validate
# Expect: either no Expected/Actual line, or one that clearly says "declared vs all-tables-in-db".
```

## Files touched (estimate)

- `crates/entry/cli/src/commands/infrastructure/jobs/run.rs` (C1 caller)
- `crates/app/scheduler/...` or wherever `bootstrap_admin_owner` lives (C1 resolver)
- `crates/entry/cli/src/commands/plugins/mcp/list_packages.rs` (C2)
- `crates/entry/cli/src/commands/web/paths.rs` (C3)
- `crates/entry/cli/src/commands/plugins/validate.rs` (C4)
- `crates/entry/cli/src/commands/plugins/mcp/mod.rs` (C5)
- `crates/entry/cli/src/commands/cloud/sync/mod.rs` (C5)
- `crates/entry/cli/src/commands/admin/access_control.rs` + `CommandResult` renderer (C6)
- `crates/entry/cli/src/lib.rs` (C7 default level)
- `crates/shared/extension/src/registry/discovery.rs` (C7 emit)
- `crates/app/runtime/src/builder.rs` (C7 emit)
- `crates/entry/cli/src/commands/infrastructure/db/schema.rs` (C8)

## Order

C1 (functional fix, blocks the most users) → C2 (single broken subcommand) → C3 (broken path) → C4 + C5 + C8 (string/clap fixes; can batch) → C6 (renderer change, slightly broader) → C7 (default log level — verify no test depends on INFO output first) → C9 (legacy sweep).

C5 and C8 can land in parallel — they're independent.

## Out of scope

- Adding new CLI commands.
- Refactoring `CommandResult`'s entire variant taxonomy (C6 adds one variant; broader cleanup is separate).
- Touching destructive job runners or services (start/stop/restart) — read-only smoke test focused on discovery/output.
- Template-side items — see `/var/www/html/systemprompt-template/CLI_FRICTION_PLAN.md`.
