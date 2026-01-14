# Skills CLI Tech Debt

**Module:** `crates/entry/cli/src/commands/skills/`
**Audit Date:** 2025-01-14
**Status:** Functional with issues

---

## Summary

The skills CLI module is largely functional and compliant with CLI standards. However, there is one critical bug, several README documentation mismatches, and missing features that were documented but not implemented.

---

## Critical Issues

### 1. Skill Detail Lookup Bug

**File:** `list.rs:63`
**Severity:** Critical
**Impact:** `skills list <name>` always fails

```rust
// Current (broken)
let skill_dir = skills_path.join(skill_id.replace('_', "-"));

// Problem: Directories use underscores (blog_writing),
// but code converts underscores to hyphens (blog-writing)
```

**Fix:**
```rust
// Option A: Use skill_id directly (directories use underscores)
let skill_dir = skills_path.join(skill_id);

// Option B: Try both formats
let skill_dir = skills_path.join(&skill_id);
if !skill_dir.exists() {
    let alt_dir = skills_path.join(skill_id.replace('_', "-"));
    if alt_dir.exists() {
        return show_skill_from_dir(&alt_dir, skill_id);
    }
}
```

**Test:**
```bash
sp skills list blog_writing  # Currently fails, should show detail
```

---

## README Discrepancies

### Documentation vs Implementation Mismatch

| Command | README Documents | Actually Implemented |
|---------|------------------|---------------------|
| `skills list` | `--source` (all/disk/database) | Not implemented |
| `skills list` | `--agent` filter | Not implemented |
| `skills create` | `--agent` (required) | Not implemented |
| `skills create` | `--prompt` / `--prompt-file` | Named `--instructions` / `--instructions-file` |
| `skills edit` | `--prompt` / `--prompt-file` | Named `--instructions` / `--instructions-file` |
| `skills status` | `--agent` filter | Not implemented |
| `skills sync` | `--agent` filter | Not implemented |
| `skills sync` | `--direction from-db` | Named `to-disk` |

### Resolution Options

**Option A: Update README to match implementation**
- Rename `--prompt` references to `--instructions`
- Remove undocumented `--source` and `--agent` flags
- Update sync direction naming

**Option B: Update implementation to match README**
- Add `--source` filter to list command
- Add `--agent` filter to list/status/sync
- Rename `--instructions` to `--prompt`
- Add agent association to skills

---

## Missing Features (Documented but Not Implemented)

### 1. Source Filtering for List

```rust
// list.rs - Add to ListArgs
#[arg(long, value_enum, default_value = "all")]
pub source: SkillSource,

#[derive(Debug, Clone, ValueEnum)]
pub enum SkillSource {
    All,
    Disk,
    Database,
}
```

### 2. Agent Filtering

```rust
// Add to ListArgs, StatusArgs, SyncArgs
#[arg(long, help = "Filter by agent name")]
pub agent: Option<String>,
```

### 3. Agent Association in Skills

Skills currently have no agent field. The frontmatter format should include:
```yaml
---
title: "My Skill"
agent: primary  # Missing field
enabled: true
---
```

---

## Code Quality Issues

### 1. Unused Config Parameter

**File:** `status.rs:22`
```rust
pub async fn execute(
    args: StatusArgs,
    _config: &CliConfig,  // Unused
) -> Result<CommandResult<SkillStatusOutput>>
```

**Fix:** Either use config for JSON output detection or document why it's unused.

### 2. Inconsistent ID Normalization

The codebase has inconsistent handling of skill IDs:

| Location | Behavior |
|----------|----------|
| `list.rs:143` | Converts `-` to `_` when reading directory names |
| `list.rs:63` | Converts `_` to `-` when looking up |
| `edit.rs:48` | Converts `_` to `-` for alternate lookup |
| `delete.rs:112` | Converts `_` to `-` for alternate lookup |

**Recommendation:** Standardize on one format (underscores) and normalize consistently.

### 3. Duplicate Helper Functions

`get_skills_path()` is duplicated in every file:
- `list.rs:57`
- `create.rs:114`
- `edit.rs:140`
- `delete.rs:101`
- `status.rs:112`
- `sync.rs:163`

**Fix:** Move to `mod.rs` or create a shared `utils.rs`.

### 4. Duplicate Skill Scanning Logic

`list_all_skills()` pattern duplicated in:
- `delete.rs:128-154`
- `edit.rs:230-250`

**Fix:** Extract to shared function in `mod.rs`.

---

## Database Cleanup

The database contains 254 orphaned E2E test skills that should be cleaned up:

```bash
# View orphaned skills
sp skills status | grep "db-only"

# Clean up (when implemented)
sp skills sync --direction to-db --delete-orphans --yes
```

---

## Compliance Checklist

| Requirement | Status |
|-------------|--------|
| All `execute` functions accept `config: &CliConfig` | PASS |
| All commands return `CommandResult<T>` | PASS |
| `delete` command requires `--yes` / `-y` flag | PASS |
| All output types derive `Serialize`, `Deserialize`, `JsonSchema` | PASS |
| No `println!` / `eprintln!` - uses `render_result()` | PASS |
| No `unwrap()` / `expect()` - uses `?` with `.context()` | PASS |
| JSON output supported via `--json` flag | PASS |
| Proper error messages for missing required flags | PASS |

---

## Enhancement Opportunities

### 1. Add Skill Validation Command

```bash
sp skills validate <name>     # Validate single skill
sp skills validate --all      # Validate all skills
```

Check for:
- Valid YAML frontmatter
- Required fields (title)
- File permissions
- Referenced assets exist

### 2. Add Skill Export/Import

```bash
sp skills export <name> > skill.yaml
sp skills import < skill.yaml
sp skills export --all > skills-backup.tar.gz
```

### 3. Add Skill Templates

```bash
sp skills create --template coding-assistant
sp skills create --template writing-assistant
sp skills templates list
```

### 4. Add Pagination

```bash
sp skills list --limit 20 --offset 40
sp skills list --page 3 --per-page 20
```

### 5. Add Search

```bash
sp skills search "blog"
sp skills list --filter "enabled=true"
```

### 6. Separate Show Subcommand

```bash
# Instead of overloading list
sp skills show blog_writing
sp skills show blog_writing --full  # Include full instructions
```

---

## Priority Matrix

| Issue | Priority | Effort | Impact |
|-------|----------|--------|--------|
| Fix skill detail lookup bug | P0 | Low | High |
| Resolve README/implementation mismatch | P1 | Medium | Medium |
| Extract duplicate helper functions | P2 | Low | Low |
| Implement `--source` filter | P2 | Medium | Medium |
| Implement `--agent` filter | P2 | Medium | Medium |
| Add validation command | P3 | Medium | Medium |
| Add export/import | P3 | High | Medium |
| Database cleanup | P3 | Low | Low |

---

## Implementation Plan

### Phase 1: Critical Fixes
1. Fix skill detail lookup bug in `list.rs:63`
2. Update README to match actual flag names

### Phase 2: Code Quality
1. Extract `get_skills_path()` to shared location
2. Extract `list_all_skills()` to shared location
3. Standardize skill ID normalization

### Phase 3: Feature Parity
1. Implement `--source` filter for list
2. Implement `--agent` filter for list/status/sync
3. Add agent field to skill frontmatter

### Phase 4: Enhancements
1. Add `skills validate` command
2. Add `skills show` subcommand
3. Add pagination to list
