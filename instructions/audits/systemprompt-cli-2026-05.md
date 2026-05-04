# systemprompt-cli Tech Debt Audit

**Layer:** entry
**Audited:** 2026-05-04
**Verdict:** CRITICAL

---

## Summary

| Category | Count |
|----------|-------|
| unwrap()/expect() | 0 |
| panic!()/todo!()/unimplemented!() | 0 |
| println!/eprintln!/dbg! | 0 |
| `let _ =` discards | 5 |
| `.ok()` discards | 73 |
| Inline `//` comments | 0 |
| Doc `///` comments | 0 |
| Files >300 lines | 17 |
| Raw String IDs | 0 |
| Raw `sqlx::query` (outside allowlist) | 3 |
| `*Manager` suffix | 0 |
| `#[allow(...)]` | 1 |
| `anyhow::` references | 530 |
| `async_trait` references | 5 |

**Total scored violations:** 99

---

## Architectural Compliance

Layer: `entry`. Per `instructions/information/boundaries.md` dependencies must flow downward only. This audit does not flag legitimate downward orchestration dependencies.

---

## Passing Checks

| Check | Status |
|-------|--------|
| No `unwrap()` / `expect()` | PASS |
| No `panic!()` / `todo!()` / `unimplemented!()` | PASS |
| No `println!` / `eprintln!` / `dbg!` | PASS |
| No `let _ =` patterns | FAIL (5) |
| No inline `//` comments | PASS |
| No `///` doc comments | PASS |
| All files <=300 lines | FAIL (17) |
| No raw String IDs | PASS |
| No raw `sqlx::query` outside allowlist | FAIL (3) |
| No `*Manager` suffix | PASS |
| No `#[allow(...)]` attributes | FAIL (1) |

---

## File Statistics

| Metric | Value |
|--------|-------|
| Total .rs files | 447 |
| Files over 300 lines | 17 |
| Largest file | `    400 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/admin/session/login.rs` |

### Files over 300 lines

```
    364 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/core/skills/create.rs
    325 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/core/content/edit.rs
    346 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/cloud/deploy/mod.rs
    386 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/cloud/deploy/pre_sync.rs
    319 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/cloud/init/mod.rs
    388 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/admin/setup/wizard.rs
    340 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/admin/setup/docker.rs
    314 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/admin/agents/tools.rs
    305 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/admin/agents/logs.rs
    348 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/admin/agents/message.rs
    340 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/admin/agents/edit.rs
    400 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/admin/session/login.rs
    338 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/plugins/mcp/tools.rs
    361 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/plugins/mcp/logs.rs
    307 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/plugins/mcp/call.rs
    325 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/infrastructure/services/mod.rs
    363 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/infrastructure/db/admin.rs
```

---

## Offending Locations

### let _ = (fire-and-forget)

```
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/core/files/upload.rs:57:        let _ = write!(acc, "{b:02x}");
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/admin/agents/message_streaming.rs:60:                                    let _ = std::io::Write::write_all(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/admin/agents/message_streaming.rs:64:                                    let _ = std::io::Write::flush(&mut std::io::stdout());
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/admin/agents/message.rs:204:                                    let _ = std::io::Write::write_all(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/admin/agents/message.rs:208:                                    let _ = std::io::Write::flush(&mut std::io::stdout());
```

### .ok() (silent error discard — verify each has logging)

```
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/bootstrap.rs:105:    let content = std::fs::read_to_string(profile_path).ok()?;
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/bootstrap.rs:106:    let profile: Profile = serde_yaml::from_str(&content).ok()?;
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/bootstrap.rs:169:        .ok()
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/session/store.rs:8:    let profile = ProfileBootstrap::get().ok();
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/core/content/verify.rs:78:                .ok()
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/web/assets/list.rs:72:        let modified = metadata.modified().ok().map_or_else(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/web/assets/show.rs:38:    let modified = metadata.modified().ok().map_or_else(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/web/sitemap/generate.rs:44:            .ok()
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/core/plugins/generate/mcp.rs:41:        .ok()?;
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/core/plugins/generate/mcp.rs:47:        .ok()?;
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/core/plugins/generate/mcp.rs:53:        .and_then(|p| u16::try_from(p).ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/core/skills/create.rs:345:        .ok()
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/infrastructure/services/serve.rs:34:                .ok();
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/infrastructure/services/serve.rs:39:                .ok();
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/infrastructure/services/serve.rs:61:        tx.unbounded_send(StartupEvent::DatabaseValidated).ok();
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/infrastructure/services/cleanup.rs:147:            service_mgmt.mark_service_stopped(&service.name).await.ok();
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/core/skills/sync.rs:182:        .ok()
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/infrastructure/logs/search.rs:112:                    .ok()
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/plugins/mcp/logs.rs:73:        .ok()
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/plugins/mcp/logs.rs:74:        .and_then(|p| AppPaths::from_profile(&p.paths).ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/infrastructure/logs/audit_display.rs:76:            .ok()
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/infrastructure/logs/audit_display.rs:77:            .and_then(|v| serde_json::to_string_pretty(&v).ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/plugins/mcp/status.rs:66:            .ok()
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/cloud/sync/skills.rs:26:        .ok()
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/cloud/sync/interactive.rs:122:            let profile = ProfileLoader::load_from_path(&profile_yaml).ok()?;
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/admin/session/list.rs:34:        SessionStore::load_or_create(&dir).ok()
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/admin/session/list.rs:94:    ProfileLoader::load_from_path(config_path).ok()
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/cloud/init/mod.rs:302:        std::fs::remove_dir_all(&git_dir).ok();
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/cloud/dockerfile/builder.rs:25:            .ok()
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/cloud/dockerfile/builder.rs:32:                    .ok()
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/cloud/dockerfile/builder.rs:151:            .filter_map(|ext| ext.path.strip_prefix(self.project_root).ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/plugins/mcp/list.rs:24:    let project_root = ProjectRoot::discover().ok();
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/plugins/mcp/list.rs:109:        .ok()
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/plugins/mcp/list.rs:110:        .and_then(|m| m.modified().ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/admin/session/show.rs:112:    let store = SessionStore::load_or_create(&sessions_dir).ok()?;
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/admin/session/show.rs:147:    let store = TenantStore::load_from_path(&tenants_path).ok()?;
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/cloud/db/backup.rs:87:            .filter_map(|e| e.metadata().ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/cloud/tenant/select.rs:60:        .ok()
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/plugins/mcp/tools_display.rs:101:                .ok();
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/plugins/mcp/tools_display.rs:105:                    .ok()
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/analytics/shared/time.rs:8:        .and_then(|d| d.parse::<i64>().ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/analytics/shared/time.rs:12:                .and_then(|h| h.parse::<i64>().ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/analytics/shared/time.rs:17:                .and_then(|m| m.parse::<i64>().ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/analytics/shared/time.rs:22:                .and_then(|w| w.parse::<i64>().ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/admin/agents/tools_mcp.rs:99:        .ok();
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/admin/agents/tools_mcp.rs:103:            .ok()
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/admin/agents/delete.rs:91:                .ok()
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/cloud/profile/show.rs:25:    let config = Config::get().ok().or_else(|| {
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/cloud/profile/show.rs:27:            Config::get().ok()
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/cloud/profile/show.rs:33:    let services_config = ConfigLoader::load().ok();
```

### Raw sqlx::query (outside allowlist)

```
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/infrastructure/jobs/enable.rs:32:    sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/infrastructure/jobs/disable.rs:32:    sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/infrastructure/jobs/cleanup_logs.rs:49:    let deleted_count = sqlx::query(
```

### #[allow(...)] attributes

```
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/cli/src/commands/admin/cowork/rotate_signing_key.rs:14:#[allow(clippy::print_stdout, clippy::unused_async)]
```

---

## Recommendations for Wave 1/2

- **(W1)** Replace 5 `let _ =` patterns with explicit error logging via `if let Err(e) = ...`.
- **(W2)** Audit 73 `.ok()` calls and ensure each precedes with a `tracing::warn!`/`error!` log of the dropped error.
- **(W1)** Split 17 files exceeding 300 lines into focused submodules.
- **(W1)** Convert 3 raw `sqlx::query` calls to compile-time-verified `sqlx::query!`/`query_as!`/`query_scalar!` macros (or move into the `admin/`/`postgres ext` allowlist if dynamic SQL is intentional).
- **(W2)** Remove 1 `#[allow(...)]` attributes by fixing the underlying clippy/rustc warnings.

---

## Verdict

**CRITICAL**

Other Wave 1 agents are concurrently fixing source code; final CLEAN status will be re-validated after the wave merges.
