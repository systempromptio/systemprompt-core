# CLI Validation Criteria

Deterministic rules for validating CLI command folders in `src/commands/`.

**Reference:** All code MUST comply with `instructions/rust/rust.md`. This document adds CLI-specific rules.

---

## 1. Structure Requirements

Each command folder MUST contain:

| File | Required | Purpose |
|------|----------|---------|
| `mod.rs` | YES | Module exports and routing |
| `plan.md` | YES | Compliance documentation |

**Validation:**
```bash
[ -f mod.rs ] || echo "FAIL: Missing mod.rs"
[ -f plan.md ] || echo "FAIL: Missing plan.md"
```

---

## 2. Rust Standards (from rust.md)

| Rule | Limit | Validation |
|------|-------|------------|
| File length | ≤300 lines | `wc -l < file.rs` |
| Function length | ≤75 lines | Manual review |
| Parameters | ≤5 per function | Manual review |
| Inline comments | ZERO | `grep -E "^\s*//" *.rs` must find nothing |
| Doc comments | ZERO | `grep -E "^\s*///" *.rs` must find nothing |

**Forbidden constructs:**

| Pattern | Validation |
|---------|------------|
| `unsafe` | `grep -E "unsafe\s*\{" *.rs` |
| `unwrap()` | `grep -E "\.unwrap\(\)" *.rs` (excluding `unwrap_or*`) |
| `expect()` | `grep -E "\.expect\(" *.rs` |
| `panic!` | `grep "panic!" *.rs` |
| `todo!` | `grep "todo!" *.rs` |
| `unimplemented!` | `grep "unimplemented!" *.rs` |
| `println!` | `grep "println!" *.rs` |
| `eprintln!` | `grep "eprintln!" *.rs` |
| `dbg!` | `grep "dbg!" *.rs` |

All patterns must return zero matches.

---

## 3. CLI-Specific Rules

### 3.1 Execute Function Signature

Every `execute` function MUST:
- Accept `config: &CliConfig` parameter
- Return `Result<CommandResult<T>>`

```rust
pub fn execute(cmd: Commands, config: &CliConfig) -> Result<CommandResult<OutputType>>
```

**Validation:**
```bash
grep -E "fn execute" *.rs | grep -v "CliConfig" && echo "FAIL"
grep -E "fn execute" *.rs | grep -v "CommandResult" && echo "FAIL"
```

### 3.2 Output Types

Output structs MUST derive:
```rust
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SomeOutput { ... }
```

### 3.3 CommandResult Constructors

Use `CommandResult::` constructors, NOT direct `CliService::` output methods:

| Data Shape | Constructor |
|------------|-------------|
| Multi-row data | `CommandResult::table(data)` |
| Simple array | `CommandResult::list(data)` |
| Single entity | `CommandResult::card(data)` |
| Plain text | `CommandResult::text(data)` |
| Copyable text | `CommandResult::copy_paste(data)` |
| Metrics | `CommandResult::chart(data, ChartType)` |

**Forbidden:**
```bash
grep "CliService::table" *.rs && echo "FAIL"
grep "CliService::json" *.rs | grep -v "render_result" && echo "FAIL"
```

### 3.4 Result Rendering

All command results MUST be rendered via:
```rust
render_result(&result);
```

### 3.5 Interactive Output

Use `CliService::` methods for interactive feedback (not command output):

| Purpose | Method |
|---------|--------|
| Section header | `CliService::section()` |
| Success message | `CliService::success()` |
| Warning message | `CliService::warning()` |
| Info message | `CliService::info()` |
| Error message | `CliService::error()` |

Guard interactive output:
```rust
if !config.is_json_output() {
    CliService::info("Processing...");
}
```

---

## 4. Dual-Mode Operation

Commands with interactive prompts MUST:

1. Check `config.is_interactive()` before prompting
2. Provide `--flag` equivalents for all prompts
3. Return error in non-interactive mode if required input missing

**Pattern:**
```rust
fn resolve_input<T, F>(
    value: Option<T>,
    flag_name: &str,
    config: &CliConfig,
    prompt_fn: F,
) -> Result<T>
where
    F: FnOnce() -> Result<T>,
{
    match value {
        Some(v) => Ok(v),
        None if config.is_interactive() => prompt_fn(),
        None => Err(anyhow!("--{} required in non-interactive mode", flag_name)),
    }
}
```

---

## 5. Standard Flag Patterns

```rust
#[arg(short = 'y', long, help = "Skip confirmation prompts")]
pub yes: bool,

#[arg(long, help = "Show what would happen without executing")]
pub dry_run: bool,

#[arg(long, help = "Override safety checks")]
pub force: bool,

#[arg(long, help = "Resource ID (required in non-interactive mode)")]
pub id: Option<String>,

#[arg(long, env = "API_KEY", help = "API key")]
pub api_key: Option<String>,
```

---

## 6. Validation Script

```bash
#!/bin/bash
set -e
cd "$1" || exit 1

FAIL=0

check() {
    if eval "$1" 2>/dev/null | grep -q .; then
        echo "FAIL: $2"
        FAIL=1
    fi
}

check_exists() {
    [ -f "$1" ] || { echo "FAIL: Missing $1"; FAIL=1; }
}

check_exists "mod.rs"
check_exists "plan.md"

for f in *.rs; do
    [ -f "$f" ] || continue
    lines=$(wc -l < "$f")
    [ "$lines" -gt 300 ] && { echo "FAIL: $f has $lines lines (max 300)"; FAIL=1; }
done

check 'grep -E "^\s*//" *.rs' "Inline comments found"
check 'grep -E "^\s*///" *.rs' "Doc comments found"
check 'grep -E "unsafe\s*\{" *.rs' "unsafe found"
check 'grep "todo!" *.rs' "todo! found"
check 'grep "unimplemented!" *.rs' "unimplemented! found"
check 'grep "println!" *.rs' "println! found"
check 'grep "eprintln!" *.rs' "eprintln! found"
check 'grep "panic!" *.rs' "panic! found"
check 'grep "dbg!" *.rs' "dbg! found"
check 'grep -E "\.unwrap\(\)" *.rs | grep -v "unwrap_or"' "unwrap() found"
check 'grep -E "\.expect\(" *.rs' "expect() found"
check 'grep -E "fn execute" *.rs | grep -v "CliConfig"' "execute missing CliConfig"
check 'grep -E "fn execute" *.rs | grep -v "CommandResult"' "execute missing CommandResult"
check 'grep "CliService::table" *.rs' "Direct CliService::table() found"

[ $FAIL -eq 0 ] && echo "PASS" || exit 1
```

Usage:
```bash
./validate.sh crates/entry/cli/src/commands/build
```
