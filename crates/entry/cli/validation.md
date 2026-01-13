# CLI Validation Criteria

Deterministic rules for validating any CLI command folder. Run these checks against every folder in `src/commands/` to ensure compliance.

---

## 0. Foundational Rules (MANDATORY)

### Rule 1: Rust Standards Compliance

All CLI code MUST comply with `instructions/rust/rust.md`. Key requirements:

| Requirement | Description |
|-------------|-------------|
| **File length** | Max 300 lines per source file |
| **Function length** | Max 75 lines per function |
| **Cognitive complexity** | Max 15 |
| **Parameters** | Max 5 per function |
| **No inline comments** | ZERO TOLERANCE - code documents itself through naming |
| **No doc comments** | ZERO TOLERANCE - no `///` or `//!` (except rare module docs) |
| **No unsafe** | Forbidden in this codebase |
| **No unwrap()** | Use `?`, `ok_or_else()`, or descriptive `expect()` |
| **No panic!/todo!/unimplemented!** | Return `Result` or implement |
| **No tests in source** | Move to `core/tests/` |
| **Typed identifiers** | Use `systemprompt_identifiers::*` not raw strings |
| **Idiomatic Rust** | Prefer combinators over imperative control flow |

**Validation commands:**
```bash
# Check file lengths
find . -name "*.rs" -exec wc -l {} + | awk '$1 > 300 {print "FAIL:", $2, "has", $1, "lines"}'

# Check for inline comments (should find NONE)
grep -r "^\s*//" --include="*.rs" . | grep -v "^.*:#\[" && echo "FAIL: Inline comments found"

# Check for doc comments (should find NONE except module docs)
grep -r "^\s*///" --include="*.rs" . && echo "FAIL: Doc comments found"

# Check for forbidden constructs
grep -r "unsafe\s*{" --include="*.rs" . && echo "FAIL: unsafe found"
grep -r "todo!()" --include="*.rs" . && echo "FAIL: todo! found"
grep -r "unimplemented!()" --include="*.rs" . && echo "FAIL: unimplemented! found"
```

### Rule 2: No Orphaned Files

All files MUST be semantically organized into proper folder structure. No loose files at arbitrary levels.

| Requirement | Description |
|-------------|-------------|
| **Folder structure** | Every domain has its own folder under `commands/` |
| **mod.rs required** | Each folder must have `mod.rs` exporting its contents |
| **plan.md required** | Each command folder must have a `plan.md` compliance document |
| **No stray files** | No `.rs` files floating outside proper module structure |
| **Consistent naming** | Files match their primary export (e.g., `create.rs` exports `create()`) |

**Expected structure:**
```
src/
├── commands/
│   ├── mod.rs              # Exports all command modules
│   ├── {domain}/
│   │   ├── mod.rs          # Domain routing and types
│   │   ├── plan.md         # Compliance/migration plan
│   │   ├── {subcommand}.rs # Individual command implementations
│   │   └── {subfolder}/    # Grouped subcommands if needed
│   │       ├── mod.rs
│   │       └── *.rs
├── shared/
│   ├── mod.rs
│   ├── plan.md
│   └── *.rs
├── presentation/
│   ├── mod.rs
│   ├── plan.md
│   └── *.rs
├── cli_settings.rs         # Global config (special case - re-exported from lib.rs)
├── lib.rs                  # Crate entrypoint
├── main.rs                 # Binary entrypoint
└── tui.rs                  # TUI bootstrap (special case - has tui_plan.md)
```

**Validation commands:**
```bash
# Check all command folders have mod.rs
for dir in src/commands/*/; do
    [ -f "$dir/mod.rs" ] || echo "FAIL: $dir missing mod.rs"
done

# Check all command folders have plan.md
for dir in src/commands/*/; do
    [ -f "$dir/plan.md" ] || echo "FAIL: $dir missing plan.md"
done

# Find orphaned .rs files (files not in a proper module)
find src/commands -name "*.rs" -not -name "mod.rs" | while read f; do
    dir=$(dirname "$f")
    [ -f "$dir/mod.rs" ] || echo "FAIL: Orphaned file $f (no mod.rs in $dir)"
done
```

---

## Automated Checks

### 1. Forbidden Patterns (MUST PASS)

```bash
# Run from crates/entry/cli/src/commands/{folder}/

# No println! or eprintln!
grep -r "println!" *.rs && echo "FAIL: println! found" || echo "PASS"
grep -r "eprintln!" *.rs && echo "FAIL: eprintln! found" || echo "PASS"

# No unwrap() in non-test code
grep -r "\.unwrap()" *.rs | grep -v "unwrap_or" | grep -v "unwrap_or_else" | grep -v "unwrap_or_default" && echo "FAIL: unwrap() found" || echo "PASS"

# No expect() in non-test code
grep -r "\.expect(" *.rs && echo "FAIL: expect() found" || echo "PASS"

# No panic!
grep -r "panic!" *.rs && echo "FAIL: panic! found" || echo "PASS"

# No dbg!
grep -r "dbg!" *.rs && echo "FAIL: dbg! found" || echo "PASS"
```

### 2. Required Patterns (MUST EXIST)

```bash
# All execute functions accept CliConfig
grep -r "fn execute" *.rs | grep -v "CliConfig" && echo "FAIL: execute without CliConfig" || echo "PASS"

# All command structs derive Args
grep -r "struct.*Args" *.rs | head -1
grep -r "#\[derive(Args)\]" *.rs || echo "WARN: Check Args derive"
```

### 3. Artifact-Compatible Results (MUST EXIST)

```bash
# Execute functions must return CommandResult<T>
grep -r "fn execute" *.rs | grep -v "CommandResult" && echo "FAIL: execute without CommandResult" || echo "PASS"

# Output structs must derive JsonSchema
grep -r "struct.*Output" *.rs -A3 | grep -v "JsonSchema" && echo "FAIL: Output missing JsonSchema" || echo "PASS"

# Must use artifact constructor (table/list/card/text/etc)
grep -r "CommandResult::" *.rs > /dev/null && echo "PASS: Uses CommandResult" || echo "FAIL: No CommandResult usage"

# No direct CliService::table() calls
grep -r "CliService::table" *.rs && echo "FAIL: Direct table() call found" || echo "PASS"

# No direct CliService::json() for command output (allowed for render_result)
grep -r "CliService::json" *.rs | grep -v "render_result" && echo "WARN: Direct json() call - verify context" || echo "PASS"
```

### 4. CliService Usage

```bash
# Should use CliService for output
grep -r "CliService::" *.rs > /dev/null && echo "PASS: Uses CliService" || echo "WARN: No CliService usage found"
```

---

## Manual Review Checklist

### A. Dual-Mode Operation

For each command in the folder:

| Check | Requirement |
|-------|-------------|
| [ ] | Command accepts `config: &CliConfig` parameter |
| [ ] | Command checks `config.is_interactive()` before prompting |
| [ ] | All interactive prompts have `--flag` equivalents |
| [ ] | Non-interactive mode returns error if required input missing |

### B. Flag Coverage

For each interactive prompt (dialoguer usage):

| Check | Requirement |
|-------|-------------|
| [ ] | `Select` prompt has corresponding `--option` flag |
| [ ] | `Input` prompt has corresponding `--value` flag |
| [ ] | `Password` prompt has corresponding `--secret` flag or env var |
| [ ] | `Confirm` prompt has corresponding `--yes`/`-y` flag |

### C. Output Standards

| Check | Requirement |
|-------|-------------|
| [ ] | Section headers use `CliService::section()` |
| [ ] | Subsections use `CliService::subsection()` |
| [ ] | Success messages use `CliService::success()` |
| [ ] | Errors use `CliService::error()` |
| [ ] | Warnings use `CliService::warning()` |
| [ ] | Info messages use `CliService::info()` |
| [ ] | Key-value pairs use `CliService::key_value()` |
| [ ] | JSON output uses `CliService::json()` |

### D. Error Handling

| Check | Requirement |
|-------|-------------|
| [ ] | All functions return `Result<T>` |
| [ ] | Errors include context via `.context()` |
| [ ] | No silent failures (empty `Ok(())` on error conditions) |
| [ ] | Non-interactive errors suggest flag alternatives |

### E. Args Struct Standards

| Check | Requirement |
|-------|-------------|
| [ ] | All flags have `#[arg(long, help = "...")]` |
| [ ] | Short flags only for common options (`-y`, `-v`, `-q`) |
| [ ] | Environment variable fallback for secrets: `#[arg(env = "...")]` |
| [ ] | Default values documented: `#[arg(default_value = "...")]` |
| [ ] | Value enums use `#[arg(value_enum)]` |

### F. Artifact-Compatible Results (MANDATORY)

| Check | Requirement |
|-------|-------------|
| [ ] | `execute()` returns `Result<CommandResult<T>>` not `Result<()>` |
| [ ] | Output type `T` derives `Serialize, Deserialize, JsonSchema` |
| [ ] | Uses appropriate constructor: `table()`, `list()`, `card()`, `text()`, etc. |
| [ ] | Rendering hints provided via `.with_hints()` for Table/Chart types |
| [ ] | Title provided via `.with_title()` for human display |
| [ ] | No direct `CliService::table()` or `CliService::json()` calls |
| [ ] | CLI entry point uses `CliService::render_result()` |
| [ ] | Output struct fields use typed identifiers (not raw strings) |

### G. Artifact Type Selection

| Data Shape | Artifact Type | Constructor |
|------------|---------------|-------------|
| Multi-row data, lists | `Table` | `CommandResult::table(data)` |
| Simple item array | `List` | `CommandResult::list(data)` |
| Single entity detail | `PresentationCard` | `CommandResult::card(data)` |
| Plain text message | `Text` | `CommandResult::text(data)` |
| Tokens, keys to copy | `CopyPasteText` | `CommandResult::copy_paste(data)` |
| Metrics, analytics | `Chart` | `CommandResult::chart(data, ChartType::Bar)` |
| Configuration view | `Form` | `CommandResult::form(data)` |
| Multi-panel view | `Dashboard` | `CommandResult::dashboard(data)` |

---

## Standard Flag Patterns

### Confirmation Skip
```rust
#[arg(short = 'y', long, help = "Skip confirmation prompts")]
pub yes: bool,
```

### Dry Run
```rust
#[arg(long, help = "Show what would happen without executing")]
pub dry_run: bool,
```

### Force Override
```rust
#[arg(long, help = "Override safety checks")]
pub force: bool,
```

### ID Selection
```rust
#[arg(long, help = "Resource ID (required in non-interactive mode)")]
pub id: Option<String>,
```

### Secret Input
```rust
#[arg(long, env = "API_KEY", help = "API key")]
pub api_key: Option<String>,
```

---

## Resolve Input Pattern

All optional inputs requiring interactive fallback MUST use this pattern:

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
        None => Err(anyhow!("--{} is required in non-interactive mode", flag_name)),
    }
}
```

---

## Interactive-Only Commands

Commands that CANNOT support non-interactive mode (e.g., OAuth browser flow) MUST:

```rust
pub async fn execute(args: Args, config: &CliConfig) -> Result<()> {
    if !config.is_interactive() {
        return Err(anyhow!(
            "This command requires interactive mode.\n\
             Alternative: [describe non-interactive alternative if any]"
        ));
    }
    // ... interactive implementation
}
```

---

## Validation Script

Save as `validate-cli-folder.sh`:

```bash
#!/bin/bash
set -e

FOLDER=$1

if [ -z "$FOLDER" ]; then
    echo "Usage: $0 <folder-path>"
    exit 1
fi

cd "$FOLDER"

echo "=== Validating: $FOLDER ==="

FAIL=0
WARN=0

# ═══════════════════════════════════════════════════════════════════════════════
# SECTION 0: Foundational Rules (rust.md compliance)
# ═══════════════════════════════════════════════════════════════════════════════

echo ""
echo "--- Rust Standards Compliance ---"

# Check file lengths (max 300 lines)
for f in *.rs; do
    [ -f "$f" ] || continue
    lines=$(wc -l < "$f")
    if [ "$lines" -gt 300 ]; then
        echo "❌ FAIL: $f has $lines lines (max 300)"
        FAIL=1
    fi
done
echo "✅ PASS: File lengths checked"

# Check for inline comments (ZERO TOLERANCE)
if grep -r "^\s*//" *.rs 2>/dev/null | grep -v "^.*:#\[" | grep -v "^.*:#!/" > /dev/null; then
    echo "❌ FAIL: Inline comments found (ZERO TOLERANCE per rust.md)"
    grep -r "^\s*//" *.rs | grep -v "^.*:#\[" | grep -v "^.*:#!/" | head -5
    FAIL=1
else
    echo "✅ PASS: No inline comments"
fi

# Check for doc comments (ZERO TOLERANCE)
if grep -r "^\s*///" *.rs 2>/dev/null > /dev/null; then
    echo "❌ FAIL: Doc comments found (ZERO TOLERANCE per rust.md)"
    grep -r "^\s*///" *.rs | head -5
    FAIL=1
else
    echo "✅ PASS: No doc comments"
fi

# Check for unsafe
if grep -r "unsafe\s*{" *.rs 2>/dev/null > /dev/null; then
    echo "❌ FAIL: unsafe blocks found (forbidden per rust.md)"
    FAIL=1
else
    echo "✅ PASS: No unsafe blocks"
fi

# Check for todo!/unimplemented!
if grep -rE "(todo!|unimplemented!)" *.rs 2>/dev/null > /dev/null; then
    echo "❌ FAIL: todo!/unimplemented! found"
    FAIL=1
else
    echo "✅ PASS: No todo!/unimplemented!"
fi

echo ""
echo "--- Structure Compliance ---"

# Check for mod.rs
if [ ! -f "mod.rs" ]; then
    echo "❌ FAIL: Missing mod.rs"
    FAIL=1
else
    echo "✅ PASS: mod.rs exists"
fi

# Check for plan.md
if [ ! -f "plan.md" ]; then
    echo "❌ FAIL: Missing plan.md"
    FAIL=1
else
    echo "✅ PASS: plan.md exists"
fi

# ═══════════════════════════════════════════════════════════════════════════════
# SECTION 1: CLI-Specific Forbidden Patterns
# ═══════════════════════════════════════════════════════════════════════════════

echo ""
echo "--- CLI Forbidden Patterns ---"

# Check for println!
if grep -r "println!" *.rs 2>/dev/null | grep -v "#\[allow" > /dev/null; then
    echo "❌ FAIL: println! found"
    grep -r "println!" *.rs | grep -v "#\[allow"
    FAIL=1
else
    echo "✅ PASS: No println!"
fi

# Check for eprintln!
if grep -r "eprintln!" *.rs 2>/dev/null > /dev/null; then
    echo "❌ FAIL: eprintln! found"
    FAIL=1
else
    echo "✅ PASS: No eprintln!"
fi

# Check for unwrap()
if grep -r "\.unwrap()" *.rs 2>/dev/null | grep -v "unwrap_or" | grep -v "unwrap_or_else" | grep -v "unwrap_or_default" > /dev/null; then
    echo "❌ FAIL: unwrap() found"
    grep -r "\.unwrap()" *.rs | grep -v "unwrap_or" | grep -v "unwrap_or_else" | grep -v "unwrap_or_default"
    FAIL=1
else
    echo "✅ PASS: No unwrap()"
fi

# Check for expect()
if grep -r "\.expect(" *.rs 2>/dev/null > /dev/null; then
    echo "❌ FAIL: expect() found"
    FAIL=1
else
    echo "✅ PASS: No expect()"
fi

# Check for panic!
if grep -r "panic!" *.rs 2>/dev/null > /dev/null; then
    echo "❌ FAIL: panic! found"
    FAIL=1
else
    echo "✅ PASS: No panic!"
fi

# Check for dbg!
if grep -r "dbg!" *.rs 2>/dev/null > /dev/null; then
    echo "❌ FAIL: dbg! found"
    FAIL=1
else
    echo "✅ PASS: No dbg!"
fi

# Check for CliService usage
if grep -r "CliService::" *.rs 2>/dev/null > /dev/null; then
    echo "✅ PASS: Uses CliService"
else
    echo "⚠️  WARN: No CliService usage found"
fi

# Check for CliConfig in execute functions
if grep -r "fn execute" *.rs 2>/dev/null | grep -v "CliConfig" > /dev/null; then
    echo "⚠️  WARN: Some execute functions may be missing CliConfig"
fi

# Check for CommandResult returns
if grep -r "fn execute" *.rs 2>/dev/null | grep -v "CommandResult" > /dev/null; then
    echo "❌ FAIL: execute functions must return CommandResult<T>"
    grep -r "fn execute" *.rs | grep -v "CommandResult"
    FAIL=1
else
    echo "✅ PASS: All execute functions return CommandResult"
fi

# Check for JsonSchema on Output structs
if grep -r "struct.*Output" *.rs 2>/dev/null > /dev/null; then
    if ! grep -r "JsonSchema" *.rs 2>/dev/null > /dev/null; then
        echo "❌ FAIL: Output structs missing JsonSchema derive"
        FAIL=1
    else
        echo "✅ PASS: Output structs have JsonSchema"
    fi
fi

# Check for direct CliService::table calls (forbidden)
if grep -r "CliService::table" *.rs 2>/dev/null > /dev/null; then
    echo "❌ FAIL: Direct CliService::table() calls found - use CommandResult::table()"
    grep -r "CliService::table" *.rs
    FAIL=1
else
    echo "✅ PASS: No direct CliService::table() calls"
fi

# Check for CommandResult usage
if grep -r "CommandResult::" *.rs 2>/dev/null > /dev/null; then
    echo "✅ PASS: Uses CommandResult constructors"
else
    echo "❌ FAIL: No CommandResult usage found"
    FAIL=1
fi

echo ""
if [ $FAIL -eq 0 ]; then
    echo "=== VALIDATION PASSED ==="
else
    echo "=== VALIDATION FAILED ==="
    exit 1
fi
```

Usage:
```bash
chmod +x validate-cli-folder.sh
./validate-cli-folder.sh crates/entry/cli/src/commands/agents
./validate-cli-folder.sh crates/entry/cli/src/commands/cloud
# etc.
```

---

## CI Integration

Add to CI pipeline:

```yaml
validate-cli:
  script:
    - for dir in crates/entry/cli/src/commands/*/; do
        ./validate-cli-folder.sh "$dir" || exit 1;
      done
```
