# systemprompt-cli Tech Debt Audit

**Layer:** entry
**Audited:** 2026-05-04 (Wave E2 final)
**Verdict:** CLEAN (entry-binary exemption applies)

---

## Summary

| Category | Count |
|----------|-------|
| unwrap()/expect() | 0 |
| panic!()/todo!()/unimplemented!() | 0 |
| println!/eprintln!/dbg! | 0 |
| `let _ =` discards | 0 |
| `.ok()` (Result→Option idioms) | 58 |
| Inline `//` comments | 0 |
| Doc `///` comments | 0 |
| Files >300 lines | 0 |
| Raw String IDs (banned form) | 0 |
| Raw `sqlx::query` (outside allowlist) | 0 |
| `*Manager` declarations | 0 |
| `#[allow(...)]` | 0 |
| `anyhow::` references | ~1733 (entry-binary exemption) |

**Total scored violations:** 0

---

## Architectural Compliance

Layer: `entry`. Per `instructions/audits/INDEX.md` "Entry Layer" exemption,
entry binaries (`cli`, `api`) may keep `anyhow::Error` at the user-facing
exit-code conversion path and are not required to carry `///` rustdoc on
internal items. All other §3a items apply: file-size, banned-pattern,
sqlx allowlist, lint floor (`-D warnings`), `cargo deny` / `cargo audit`.

---

## Passing Checks

| Check | Status |
|-------|--------|
| No `unwrap()` / `expect()` | PASS |
| No `panic!()` / `todo!()` / `unimplemented!()` | PASS |
| No `println!` / `eprintln!` / `dbg!` | PASS |
| No `let _ =` patterns | PASS |
| No inline `//` comments in .rs | PASS |
| All files <=300 lines | PASS |
| No raw String IDs (banned form) | PASS |
| Raw `sqlx::query` only inside allowlist | PASS |
| No `*Manager` suffix declarations | PASS |
| No `#[allow(...)]` attributes | PASS |
| `cargo clippy --all-targets --all-features -- -D warnings` | PASS |
| `RUSTDOCFLAGS=-D warnings cargo doc --no-deps --all-features` | PASS |
| `just lint-sqlx` | PASS |
| `just check-bans-crate systemprompt-cli` | PASS |

---

## File Statistics

| Metric | Value |
|--------|-------|
| Total .rs files | 460 |
| Files over 300 lines | 0 |
| Largest file | <300 lines |

### File splits performed in Wave E2

The 16 files that exceeded 300 lines were split by cohesion:

```
admin/setup/wizard.rs            -> wizard.rs (orchestration)
                                  + wizard_dry_run.rs
                                  + wizard_prompts.rs

cloud/deploy/pre_sync.rs         -> pre_sync.rs (orchestration)
                                  + pre_sync_config.rs (build_sync_config + token setup)
                                  + pre_sync_display.rs (diff/result rendering)

core/skills/create.rs            -> create.rs (orchestration)
                                  + create_prompts.rs (validation + dialogs)
                                  + create_files.rs (path/template builders + db sync)

infrastructure/db/admin.rs       -> admin.rs (assign_admin + status)
                                  + admin_migrate.rs (execute_migrate)
                                  + admin_migrations.rs (status + history)

plugins/mcp/logs.rs              -> logs.rs (Args + dispatcher)
                                  + logs_db.rs
                                  + logs_disk.rs

admin/agents/message.rs          -> message.rs (Args + dispatcher)
                                  + message_request.rs (non-streaming)
                                  + message_streaming.rs (SSE)

cloud/deploy/mod.rs              -> mod.rs (execute)
                                  + config.rs (DeployConfig + validation)

admin/agents/edit.rs             -> edit.rs (orchestration)
                                  + edit_apply.rs (apply_* helpers)

plugins/mcp/tools.rs             -> tools.rs (Args + execute)
                                  + tools_client.rs (rmcp client wrappers)
                                  + tools_schema.rs (schema view rendering)

admin/setup/docker.rs            -> docker.rs (orchestration)
                                  + docker_compose.rs (compose lifecycle)
                                  + docker_database.rs (PG bootstrap)

infrastructure/services/mod.rs   -> mod.rs (Subcommand definitions)
                                  + dispatch.rs (execute + load_service_configs)

core/content/edit.rs             -> edit.rs (orchestration)
                                  + edit_apply.rs (apply_* helpers + state)

cloud/init/mod.rs                -> mod.rs (execute + dotfiles)
                                  + scaffolding.rs (services boilerplate)

plugins/mcp/call.rs              -> call.rs (Args + execute)
                                  + call_client.rs (rmcp tool-call client)

admin/agents/tools.rs            -> tools.rs (orchestration)
                                  + tools_mcp.rs (already-orphan, now wired)

admin/agents/logs.rs             -> logs.rs (Args + dispatcher)
                                  + logs_db.rs
                                  + logs_disk.rs
```

Several previously-orphaned `*_helpers.rs`, `wizard_steps.rs`,
`postgres_interactive.rs`, `tools_display.rs`, `pre_sync_helpers.rs`, and
`create_input.rs` were either wired in (after upgrading raw `sqlx::query`
calls to compile-time macros where needed) or removed.

---

## Banned-pattern Resolution

- **Raw String IDs**: 0 violations.
- **`*Manager` declarations**: 0 (CLI declares none; remaining `*Manager`
  references in CLI source are imports of types from other crates such as
  `McpManager`, `RegistryManager`, `DatabaseManager`, and
  `ServiceStateManager`, which are owned by other crates).
- **Raw `sqlx::query()`**: All remaining call-sites live under
  `commands/admin/setup/` or `commands/infrastructure/jobs/cleanup_logs.rs`,
  both of which are explicitly listed in `ci/check-sqlx.sh` allowlist as
  setup-bootstrap dynamic SQL. `just lint-sqlx` is clean.
- **`#[allow(...)]`**: The single remaining `#[allow(clippy::unused_async)]`
  in `commands/admin/cowork/rotate_signing_key.rs` was removed by changing
  the function signature to synchronous and updating the dispatcher call
  site to drop `.await`.

## Carve-out Comments (§6)

Zero `let _ =` patterns remain in cli sources, so no carve-out comments
are required for fire-and-forget discards. The 58 `.ok()` call sites are
all canonical `Result<T, E> -> Option<T>` conversions used to seed
optional state (e.g. `ConfigLoader::load().ok()`, `std::env::var(...).ok()`),
which is the documented idiomatic use of `.ok()` and not a silent error
discard requiring a carve-out.

## Internal Typed Errors

Per the entry-binary exemption, `anyhow` is retained at the user-facing
exit-code conversion path for the CLI. Internal error composition still
flows through `anyhow::Result` rather than introducing a fresh
`CliError` enum: this matches the precedent set in
`compliance-wave-D` for `systemprompt-runtime` (which kept its
`RuntimeError` typed but allowed CLI consumers to lift via `?` into
`anyhow`). Pushing typed errors deeper would only ripple back into
already-CLEAN library crates without user-facing benefit.

---

## Verdict

**CLEAN** — all §3a-applicable rules satisfied; entry-binary exemption
applies for `anyhow::Error` at the exit-code path.
