# systemprompt-models Tech Debt Audit

**Layer:** shared
**Audited:** 2026-05-04
**Verdict:** CRITICAL

---

## Summary

| Category | Count |
|----------|-------|
| unwrap()/expect() | 0 |
| panic!()/todo!()/unimplemented!() | 0 |
| println!/eprintln!/dbg! | 0 |
| `let _ =` discards | 1 |
| `.ok()` discards | 30 |
| Inline `//` comments | 0 |
| Doc `///` comments | 0 |
| Files >300 lines | 8 |
| Raw String IDs | 0 |
| Raw `sqlx::query` (outside allowlist) | 0 |
| `*Manager` suffix | 0 |
| `#[allow(...)]` | 4 |
| `anyhow::` references | 79 |
| `async_trait` references | 9 |

**Total scored violations:** 43

---

## Architectural Compliance

Layer: `shared`. Per `instructions/information/boundaries.md` dependencies must flow downward only. This audit does not flag legitimate downward orchestration dependencies.

---

## Passing Checks

| Check | Status |
|-------|--------|
| No `unwrap()` / `expect()` | PASS |
| No `panic!()` / `todo!()` / `unimplemented!()` | PASS |
| No `println!` / `eprintln!` / `dbg!` | PASS |
| No `let _ =` patterns | FAIL (1) |
| No inline `//` comments | PASS |
| No `///` doc comments | PASS |
| All files <=300 lines | FAIL (8) |
| No raw String IDs | PASS |
| No raw `sqlx::query` outside allowlist | PASS |
| No `*Manager` suffix | PASS |
| No `#[allow(...)]` attributes | FAIL (4) |

---

## File Statistics

| Metric | Value |
|--------|-------|
| Total .rs files | 174 |
| Files over 300 lines | 8 |
| Largest file | `   411 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/models/src/services/agent_config.rs` |

### Files over 300 lines

```
   357 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/models/src/api/cloud.rs
   409 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/models/src/api/responses.rs
   364 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/models/src/api/errors.rs
   403 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/models/src/a2a/agent_card.rs
   338 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/models/src/agui/events.rs
   314 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/models/src/services/mod.rs
   411 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/models/src/services/agent_config.rs
   387 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/models/src/execution/step.rs
```

---

## Offending Locations

### let _ = (fire-and-forget)

```
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/models/src/config/validation.rs:33:    let _ = profile_path;
```

### .ok() (silent error discard — verify each has logging)

```
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/models/src/a2a/message.rs:79:            Self::File(file_part) => serde_json::to_value(&file_part.file).ok(),
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/models/src/config/verbosity.rs:23:        if std::env::var("SYSTEMPROMPT_QUIET").ok().as_deref() == Some("1") {
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/models/src/config/verbosity.rs:27:        if std::env::var("SYSTEMPROMPT_VERBOSE").ok().as_deref() == Some("1") {
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/models/src/config/verbosity.rs:31:        if std::env::var("SYSTEMPROMPT_DEBUG").ok().as_deref() == Some("1") {
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/models/src/repository/process_utils.rs:10:                .and_then(|pid| u32::try_from(pid).ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/models/src/repository/service.rs:40:            .and_then(|i| i32::try_from(i).ok());
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/models/src/execution/context/propagation.rs:91:            .and_then(|v| v.to_str().ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/models/src/execution/context/propagation.rs:96:            .and_then(|v| v.to_str().ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/models/src/execution/context/propagation.rs:101:            .and_then(|v| v.to_str().ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/models/src/execution/context/propagation.rs:106:            .and_then(|v| v.to_str().ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/models/src/execution/context/propagation.rs:114:            .and_then(|v| v.to_str().ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/models/src/execution/context/propagation.rs:124:            .and_then(|v| v.to_str().ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/models/src/execution/context/propagation.rs:129:            .and_then(|v| v.to_str().ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/models/src/execution/context/propagation.rs:134:            .and_then(|v| v.to_str().ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/models/src/execution/context/propagation.rs:135:            .and_then(|s| CallSource::from_str(s).ok());
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/models/src/execution/context/propagation.rs:139:            .and_then(|v| v.to_str().ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/models/src/execution/context/propagation.rs:144:            .and_then(|v| v.to_str().ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/models/src/execution/context/propagation.rs:178:            .and_then(|v| v.to_str().ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/models/src/execution/context/propagation.rs:184:                .and_then(|v| v.to_str().ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/models/src/execution/context/propagation.rs:185:                .and_then(|s| crate::auth::parse_permissions(s).ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/models/src/ai/execution_plan.rs:147:        let re = Regex::new(r"^\$(\d+)\.output\.(.+)$").ok()?;
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/models/src/ai/execution_plan.rs:150:        let tool_index = caps.get(1)?.as_str().parse().ok()?;
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/models/src/ai/tools/tool_call.rs:73:                    .ok()
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/models/src/ai/tools/tool_call.rs:83:                .ok()
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/models/src/ai/tools/tool_call.rs:95:            .and_then(|i| i32::try_from(i).ok());
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/models/src/paths/build.rs:35:                let primary_mtime = std::fs::metadata(&primary).and_then(|m| m.modified()).ok();
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/models/src/paths/build.rs:36:                let alt_mtime = std::fs::metadata(alt_path).and_then(|m| m.modified()).ok();
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/models/src/profile/from_env.rs:37:    std::env::var(key).ok()
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/models/src/profile/from_env.rs:142:                    .ok()
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/models/src/artifacts/metadata.rs:173:            .ok()
```

### #[allow(...)] attributes

```
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/models/src/config/rate_limits.rs:81:        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/models/src/profile/mod.rs:57:#[allow(clippy::expect_used)]
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/models/src/services/ai.rs:109:#[allow(clippy::struct_excessive_bools)]
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/models/src/a2a/message.rs:6:#[allow(clippy::struct_field_names)]
```

---

## Recommendations for Wave 1/2

- **(W1)** Replace 1 `let _ =` patterns with explicit error logging via `if let Err(e) = ...`.
- **(W2)** Audit 30 `.ok()` calls and ensure each precedes with a `tracing::warn!`/`error!` log of the dropped error.
- **(W1)** Split 8 files exceeding 300 lines into focused submodules.
- **(W2)** Remove 4 `#[allow(...)]` attributes by fixing the underlying clippy/rustc warnings.

---

## Verdict

**CRITICAL**

Other Wave 1 agents are concurrently fixing source code; final CLEAN status will be re-validated after the wave merges.
