# Files CLI Tech Debt Report

**Generated:** 2026-01-14
**Module:** `crates/entry/cli/src/commands/files/`

---

## Executive Summary

The files CLI module is **mostly functional** with good adherence to code quality standards (no `println!`, `unwrap()`, or `expect()`). However, there are **7 violations** against the main CLI README requirements and **documentation drift** that needs addressing.

---

## Test Results

### Commands Tested

| Command | Status | Notes |
|---------|--------|-------|
| `files list` | ✅ Pass | Pagination and filtering work correctly |
| `files show` | ✅ Pass | Returns detailed file info with metadata |
| `files upload` | ✅ Pass | Context required, file validation works |
| `files delete` | ⚠️ Issue | Missing `--yes` enforcement in non-interactive |
| `files validate` | ✅ Pass | Validates file before upload |
| `files config` | ✅ Pass | Shows upload configuration |
| `files content list` | ⚠️ Issue | Positional arg vs documented `--content` flag |
| `files content link` | ⚠️ Issue | `--role` vs documented `--type`, different values |
| `files content unlink` | ⚠️ Issue | Missing `--yes` flag for destructive operation |
| `files content featured` | ⚠️ Issue | Undocumented command |
| `files ai list` | ✅ Pass | Lists AI-generated images |
| `files ai count` | ⚠️ Issue | `--user` required but marked optional |

---

## Violations

### 1. `files delete` - Missing `--yes` Enforcement (CRITICAL)

**Location:** `delete.rs:35`

**Current behavior:**
```rust
if !args.yes && config.is_interactive() {
    // Confirmation only shown in interactive mode
}
```

**Problem:** In non-interactive mode, the command proceeds without `--yes`, violating the README requirement that destructive operations MUST require `--yes` in non-interactive mode.

**Required fix:**
```rust
if !args.yes {
    if config.is_interactive() {
        // Show confirmation dialog
    } else {
        return Err(anyhow!("--yes is required to delete files in non-interactive mode"));
    }
}
```

---

### 2. `files content unlink` - Missing `--yes` Flag (CRITICAL)

**Location:** `content/unlink.rs`

**Problem:** The `unlink` command is a destructive operation but has no `--yes` flag at all.

**Required fix:** Add `--yes` flag and confirmation logic matching `delete.rs`.

---

### 3. `files content list` - API Mismatch

**Location:** `content/list.rs:12-15`

**Current:** Positional argument `<CONTENT_ID>`
```bash
sp files content list content_abc123
```

**Documented:** Flag `--content`
```bash
sp files content list --content content_abc123
```

**Required fix:** Either update the README or change to use `--content` flag for consistency with other commands.

---

### 4. `files content link` - Flag Name Mismatch

**Location:** `content/link.rs:40`

**Current:** `--role` flag with values: `featured`, `attachment`, `inline`, `og-image`, `thumbnail`

**Documented:** `--type` flag with values: `attachment`, `thumbnail`, `preview`

**Required fix:** Update README to reflect actual `--role` flag and values, or rename flag to match docs.

---

### 5. `files ai count` - Misleading Optional Parameter

**Location:** `ai/count.rs:13-15,26-32`

**Current:**
```rust
#[arg(long, help = "Filter by user ID")]
pub user: Option<String>,  // Marked optional

// But code requires it:
None => return Err(anyhow!("User ID is required for counting AI images. Use --user flag."));
```

**Problem:** The `--user` flag appears optional in help but is actually required.

**Required fix:** Either implement counting without user filter, or make `--user` required in the Args definition:
```rust
#[arg(long, required = true, help = "User ID (required)")]
pub user: String,
```

---

### 6. `files content featured` - Undocumented Command

**Location:** `content/featured.rs`

**Problem:** The `featured` subcommand exists and works but is not documented in the README.

**Required fix:** Add documentation:
```markdown
### files content featured

Get or set the featured image for content.

\`\`\`bash
sp files content featured <content-id>
sp files content featured <content-id> --set <file-id>
\`\`\`
```

---

### 7. JSON Output Structure Drift

**Locations:** Various commands

**Problem:** The README documents simplified JSON output, but actual output includes wrapper structure with `data`, `artifact_type`, `title`, and `hints` fields.

**Example - README shows:**
```json
{
  "files": [...],
  "total": 1
}
```

**Actual output:**
```json
{
  "data": {
    "files": [...],
    "total": 1
  },
  "artifact_type": "table",
  "title": "Files",
  "hints": {...}
}
```

**Required fix:** Update README to show actual output structure including artifact metadata.

---

## Code Quality Assessment

### Positive Findings

| Check | Status |
|-------|--------|
| No `println!` | ✅ Pass |
| No `eprintln!` | ✅ Pass |
| No `unwrap()` | ✅ Pass |
| No `expect()` | ✅ Pass |
| All `execute` functions accept `config: &CliConfig` | ✅ Pass |
| Uses typed identifiers (FileId, ContentId, etc.) | ✅ Pass |
| Output types derive `Serialize`, `Deserialize`, `JsonSchema` | ✅ Pass |
| Returns `CommandResult<T>` | ✅ Pass |
| Uses proper artifact types | ✅ Pass |

### Minor Issues

1. **Unused `_config` parameter** in several commands that don't use it but accept it for compliance
2. **Error message consistency** - some use "File not found" others use more verbose messages

---

## Friction Points

### 1. Context ID Discovery
Users must know the context ID to upload files. No command to list available contexts from the files module.

**Workaround:** Users must use other CLI commands to find context IDs.

### 2. File ID Format
File IDs are UUIDs, but error messages for invalid IDs show raw UUID parsing errors rather than user-friendly messages.

**Current:**
```
invalid character: expected an optional prefix of `urn:uuid:` followed by [0-9a-fA-F-], found `n` at 1
```

**Better:**
```
Invalid file ID format. Expected UUID like 'b75940ac-c50f-4d46-9fdd-ebb4970b2a7d'
```

### 3. Bulk Operations
No bulk delete or bulk link commands. Each operation requires individual calls.

---

## Enhancement Suggestions

### High Priority

1. **Add `--yes` enforcement** for destructive operations in non-interactive mode
2. **Update README** to match actual command signatures and output structures
3. **Document `files content featured`** command

### Medium Priority

4. **Add `files content list --file <file-id>`** to list content linked to a file (reverse lookup)
5. **Improve error messages** for invalid UUID formats
6. **Add dry-run support** for `files delete` and `files content unlink`

### Low Priority (Enhancements)

7. **Add `files stats`** command to show storage usage statistics
8. **Add `files search`** command to search files by path/name pattern
9. **Add bulk operations** like `files delete-many` or `files link-many`
10. **Add `--recursive` flag** to `files content list` to show nested content
11. **Add file checksum verification** command to validate stored files
12. **Support for downloading files** via `files download <id> --output <path>`

---

## Recommended Fix Order

1. **Critical:** Fix `--yes` enforcement in `delete.rs` and add to `unlink.rs`
2. **High:** Update README documentation for accurate command signatures
3. **Medium:** Fix `files ai count` to either work without `--user` or mark it required
4. **Low:** Implement enhancement suggestions based on user feedback

---

## Compliance Checklist Update

Current README checklist claims all items are passing, but based on testing:

```markdown
- [x] All `execute` functions accept `config: &CliConfig`
- [x] All commands return `CommandResult<T>` with proper artifact type
- [ ] `delete` commands require `--yes` / `-y` flag ← FAILS
- [x] All output types derive `Serialize`, `Deserialize`, `JsonSchema`
- [x] No `println!` / `eprintln!` - uses `CliService`
- [x] No `unwrap()` / `expect()` - uses `?` with `.context()`
- [x] JSON output supported via `--json` flag
- [ ] Proper error messages for missing required flags ← PARTIAL
```
