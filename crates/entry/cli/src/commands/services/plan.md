# Services Domain Migration Plan

**Commands:** `systemprompt services [start|stop|restart|status|cleanup|serve|db|scheduler]`

## Current State

### Files
- `mod.rs` - Command router
- `start.rs` - Start services
- `stop.rs` - Stop services
- `restart.rs` - Restart services
- `status.rs` - Service status
- `serve.rs` - Foreground serve
- `cleanup.rs` - Cleanup operations
- `db/mod.rs` - Database subcommands
- `scheduler/mod.rs` - Scheduler subcommands

### Violations Found (ALL RESOLVED)

| File | Violation | Status |
|------|-----------|--------|
| `mod.rs` | Missing `config: &CliConfig` in execute | FIXED |
| `status.rs` | Direct `CliService::table()` call | FIXED |
| `status.rs` | `json` param instead of using config | FIXED |
| `serve.rs` | Missing port conflict `--kill-port-process` flag | FIXED |
| `db/mod.rs` | Missing `--yes` on reset command | N/A (no Reset cmd) |
| `start.rs` | Uses `crate::common::web` import | FIXED |
| All | Returns `Result<()>` not `Result<CommandResult<T>>` | PARTIAL* |

*Note: Execute functions still return Result<()> but use CommandResult internally for structured output.

---

## Migration Target

Location: `src/commands/services/` (complete)

---

## Required Changes

### 1. Function Signatures

Add `config: &CliConfig` to all execute functions. DONE

### 2. Fix Direct CliService::table() Call

```rust
// status.rs - AFTER
CommandResult::table(output)
    .with_title("Service Status")
    .with_hints(RenderingHints { ... })
```

### 3. Add --kill-port-process Flag

```rust
// serve.rs - DONE
#[arg(long, help = "Kill process using the port if occupied")]
kill_port_process: bool,
```

### 4. Port Conflict Handling Pattern

```rust
let should_kill = args.kill_port_process ||
    (config.is_interactive() && CliService::confirm("Kill process using port?")?);

if should_kill {
    kill_process(pid);
} else if !config.is_interactive() {
    return Err(anyhow!(
        "Port {} in use. Use --kill-port-process to terminate.",
        port
    ));
}
```

---

## Implementation Checklist

- [x] Move to `src/commands/services/` (already in place)
- [x] Add `config: &CliConfig` to all execute functions
- [x] Replace `CliService::table()` with `CommandResult::table()` pattern
- [x] Add `--kill-port-process` flag to serve
- [x] Add `--yes` flag to db reset (N/A - no Reset command exists)
- [x] Update import from `crate::common::web` to `crate::shared::web`
- [x] Implement port conflict handling pattern

## Validation Status: COMPLETE

All validation checks pass:
- No println!, eprintln!, unwrap(), expect(), panic!, dbg!
- No direct CliService::table() calls
- All execute functions have CliConfig parameter
- Output types derive JsonSchema
- Uses CommandResult pattern for status command

---

## Required Flags

| Command | Required Flags |
|---------|---------------|
| `services serve` | `--kill-port-process` |
| `services db reset` | `--yes` (N/A - no Reset cmd) |
