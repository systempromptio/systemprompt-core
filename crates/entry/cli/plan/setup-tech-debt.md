# Setup Command - Tech Debt & Violations Report

## Summary

| Category | Count |
|----------|-------|
| Critical Violations | 3 |
| Moderate Issues | 6 |
| Tech Debt Items | 8 |
| Enhancement Suggestions | 5 |

---

## Critical Violations

### 1. JSON Output Not Respected

**Location:** `setup/wizard.rs`, `setup/postgres.rs`, `setup/secrets.rs`, `setup/profile.rs`
**Violation:** README requires all commands to support `--json` output, but setup command ignores this flag completely.

**Evidence:**
```bash
./target/debug/systemprompt --non-interactive --json setup --environment test --anthropic-key "sk-ant-test123" --no-migrate
# Output is human-formatted, NOT JSON
```

**Expected:** Command should return JSON matching the structure in README:
```json
{
  "environment": "dev",
  "profile_path": "/path/to/profile.yaml",
  "database": { ... },
  "secrets_configured": { ... },
  "migrations_run": true,
  "message": "..."
}
```

**Fix Required:**
- Define `SetupOutput` struct with `Serialize, Deserialize, JsonSchema`
- Return `CommandResult<SetupOutput>` instead of `Result<()>`
- Use `CliService::render_result()` pattern

---

### 2. Returns `Result<()>` Instead of `CommandResult<T>`

**Location:** `setup/mod.rs:84`, `setup/wizard.rs:11`
**Violation:** README Part 4 (Artifact-Compatible Results) states ALL commands MUST return `CommandResult<T>`.

**Current:**
```rust
pub async fn execute(args: SetupArgs, config: &crate::CliConfig) -> Result<()>
```

**Required:**
```rust
pub async fn execute(args: SetupArgs, config: &CliConfig) -> Result<CommandResult<SetupOutput>>
```

---

### 3. Missing Output Type Definition

**Location:** `setup/` module
**Violation:** README requires all output types to derive `Serialize, Deserialize, JsonSchema`.

**Missing:** No `types.rs` file with structured output types.

**Required Types:**
```rust
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SetupOutput {
    pub environment: String,
    pub profile_path: String,
    pub database: DatabaseSetupInfo,
    pub secrets_configured: SecretsInfo,
    pub migrations_run: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DatabaseSetupInfo {
    pub host: String,
    pub port: u16,
    pub name: String,
    pub user: String,
    pub connection_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SecretsInfo {
    pub anthropic: bool,
    pub openai: bool,
    pub gemini: bool,
    pub github: bool,
}
```

---

## Moderate Issues

### 4. Password Displayed in Plain Text

**Location:** `setup/docker.rs:69`
```rust
CliService::success(&format!("Generated password: {}", password));
```

**Issue:** Passwords should not be displayed in logs. Consider masking or only showing on explicit request.

**Recommendation:** Use `CliService::info("Password generated (use --show-password to display)")` or similar.

---

### 5. `unreachable!()` Usage

**Location:**
- `setup/postgres.rs:103`
- `setup/secrets.rs:144`

**Issue:** `unreachable!()` will panic if reached. Use proper error handling.

**Fix:**
```rust
// Instead of:
_ => unreachable!()
// Use:
_ => return Err(anyhow!("Unexpected selection value"))
```

---

### 6. Hardcoded Profile Path

**Location:** `setup/wizard.rs` print_summary function

**Issue:** The example path in README shows `~/.systemprompt/profiles/` but actual path is project-relative.

**Recommendation:** Document clearly where profiles are created and why.

---

### 7. Error on Missing Services Directory

**Location:** `setup/wizard.rs:46-48`

**Issue:** Profile validation warns about missing `services` directory, but this is expected for a fresh setup.

**Recommendation:** Either create the directory during setup or make the validation smarter about new installations.

---

### 8. No Dry-Run Support

**Violation:** README Standard Flags (1.5) recommends `--dry-run` for expensive operations.

**Missing Flag:**
```rust
#[arg(long, help = "Preview setup without creating files")]
pub dry_run: bool,
```

---

### 9. No `--yes` Flag for Confirmation Skip

**Violation:** README 1.5 requires `--yes` flag for operations that modify system state.

**Missing:** Setup creates files and potentially modifies databases but has no confirmation step.

---

## Tech Debt Items

### T1. Inconsistent AI Provider Key Handling

**Location:** `setup/secrets.rs`

The README documents format validation for Anthropic keys (`sk-ant-...`) but this validation isn't implemented.

```rust
// README says:
// Warning: Anthropic API key format appears invalid (should start with sk-ant-)
// But no validation exists
```

---

### T2. Missing Config File Support

**README Documents:**
```rust
#[arg(long)]
pub config: Option<PathBuf>,  // Setup from config file
```

**Current Implementation:** Not present in `SetupArgs`.

---

### T3. Missing `--reuse-container` Flag

**README Documents:**
```rust
#[arg(long)]
pub reuse_container: bool,
```

**Current:** The flag exists implicitly in Docker flow but isn't exposed as CLI argument.

---

### T4. No Environment Variable Fallback for DB Config

**Location:** `setup/mod.rs` - `SetupArgs`

**Current:**
```rust
#[arg(long, default_value = "localhost", help = "PostgreSQL host")]
pub db_host: String,
```

**Should Match README Pattern:**
```rust
#[arg(long, env = "SYSTEMPROMPT_DB_HOST")]
pub db_host: Option<String>,
```

---

### T5. Migration Command Path Incorrect

**Location:** `setup/profile.rs:154`
```rust
.args(["services", "db", "migrate"])
```

**Actual Command:** `systemprompt db migrate` (no `services` prefix)

---

### T6. No `--skip-migrate` Flag

**README Documents `--skip-migrate`** but implementation has `--no-migrate`.

These should be consistent.

---

### T7. Docker Container Name Not Customizable

**Location:** `setup/docker.rs`

Container name is always `systemprompt-postgres-{env}`. Should be configurable for users with multiple projects.

---

### T8. No Progress Indicators for Long Operations

Docker pulls and database migrations can take significant time with no progress feedback in non-interactive mode.

---

## CLI-Wide Violations Found During Audit

### V1. `println!` Usage in Production Code

**Location:** `commands/users/export.rs:81`
```rust
println!("{}", json);
```

**Fix:** Replace with `CliService::raw(&json)`

---

### V2. Multiple Commands Missing `config: &CliConfig`

**Affected:**
- `skills/mod.rs:38` - `execute(command: SkillsCommands)`
- `agents/run.rs:15` - `execute(args: RunArgs)`
- `agents/mod.rs:62` - `execute(command: AgentsCommands)`
- `system/mod.rs:18` - `execute(command: SystemCommands)`
- `content/mod.rs:55` - `execute(command: ContentCommands)`
- `mcp/mod.rs:35` - `execute(command: McpCommands)`
- `cloud/sync/interactive.rs:27` - `execute()`
- `cloud/sync/skills.rs:28` - `execute(args: SkillsSyncArgs)`
- `cloud/sync/content/mod.rs:57` - `execute(args: ContentSyncArgs)`

---

### V3. Many Commands Return `Result<()>` Instead of `CommandResult<T>`

Over 100 execute functions return `Result<()>`. This is a gradual migration but should be tracked.

---

## Enhancement Suggestions

### E1. Add `--verify` Flag

Verify setup configuration without creating anything:
```bash
systemprompt setup --verify --environment dev --anthropic-key "..."
```

---

### E2. Add `--repair` Mode

Detect and fix common issues with existing setups:
```bash
systemprompt setup --repair --environment dev
```

---

### E3. Support Multiple AI Providers in One Command

Currently, the interactive flow suggests choosing ONE provider. Allow selecting multiple:
```bash
systemprompt setup --environment dev \
  --anthropic-key "..." \
  --openai-key "..." \
  --gemini-key "..."
```
(This actually works in non-interactive mode, but interactive flow is limited)

---

### E4. Add Profile Import/Export

```bash
systemprompt setup --export-config setup.toml  # Export current config
systemprompt setup --config setup.toml         # Import config
```

---

### E5. Add Setup Status Command

```bash
systemprompt setup status  # Show current setup status
systemprompt setup validate  # Validate existing setup
```

---

## Compliance Checklist Update

Current `setup/README.md` claims:
```
- [x] All `execute` functions accept `config: &CliConfig`  ✅ PASS
- [x] All commands return `CommandResult<T>` with proper artifact type  ❌ FAIL
- [x] All output types derive `Serialize`, `Deserialize`, `JsonSchema`  ❌ FAIL
- [x] No `println!` / `eprintln!` - uses `CliService`  ✅ PASS
- [x] No `unwrap()` / `expect()` - uses `?` with `.context()`  ✅ PASS
- [x] `resolve_input` pattern used for interactive/non-interactive selection  ✅ PASS
- [x] JSON output supported via `--json` flag  ❌ FAIL
- [x] Proper error messages for missing required flags  ✅ PASS
- [x] Environment variables supported as fallback for API keys  ✅ PASS
```

---

## Priority Order for Fixes

1. **P0 - Critical:** Implement `CommandResult<SetupOutput>` return type
2. **P0 - Critical:** Add JSON output support
3. **P1 - High:** Create `types.rs` with proper output structures
4. **P1 - High:** Fix `unreachable!()` patterns
5. **P2 - Medium:** Add `--dry-run` and `--yes` flags
6. **P2 - Medium:** Fix migration command path
7. **P3 - Low:** Add environment variable fallbacks for all DB options
8. **P3 - Low:** Implement `--config` file support
