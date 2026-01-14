# Content CLI Tech Debt & Enhancements Plan

## Overview

This plan addresses documentation drift, missing features, and inconsistencies in the content CLI commands discovered during testing.

---

## Phase 1: Code Fixes (Consistency & Bugs)

### 1.1 Fix Interactive Check Inconsistency
**Files:** `delete.rs`, `delete_source.rs`

Change `config.interactive` to `config.is_interactive()` for consistency with the rest of the codebase.

### 1.2 Make Base URL Configurable
**Files:** `link/generate.rs`, `link/show.rs`

Remove hardcoded `DEFAULT_BASE_URL` and pull from profile/config or use a constant from a shared location.

---

## Phase 2: Add Missing Flags

### 2.1 content list - Add `--category` and `--status` flags
**File:** `list.rs`

```rust
#[arg(long, help = "Filter by category ID")]
pub category: Option<String>,

#[arg(long, help = "Filter by status: draft, published, archived")]
pub status: Option<String>,
```

### 2.2 content search - Add `--source` flag
**File:** `search.rs`

```rust
#[arg(long, help = "Filter by source ID")]
pub source: Option<String>,
```

### 2.3 content ingest - Add `--dry-run` flag
**File:** `ingest.rs`

```rust
#[arg(long, help = "Preview changes without writing to database")]
pub dry_run: bool,
```

### 2.4 content popular - Change `--days` to `--since` with duration parsing
**File:** `popular.rs`

Change from integer days to duration string (e.g., "7d", "30d", "1w").

---

## Phase 3: Add Missing Commands

### 3.1 content link delete
**File:** `link/delete.rs` (new)

Add command to delete a link by ID or short code with `--yes` confirmation.

---

## Phase 4: Update README

### 4.1 Sync README with Implementation
**File:** `README.md`

- Update command reference table
- Fix analytics subcommands (views/engagement → clicks/campaign/journey)
- Fix link subcommands (create/delete → generate/show/performance)
- Document frontmatter requirements for ingest
- Update flag documentation

---

## Implementation Checklist

- [x] 1.1 Fix `config.interactive` → `config.is_interactive()`
- [x] 2.1 Add `--category` to `content list`
- [x] 2.2 Add `--source` to `content search`
- [x] 2.3 Add `--dry-run` to `content ingest`
- [x] 2.4 Change `--days` to `--since` in `content popular`
- [x] 3.1 Add `content link delete` command
- [x] 4.1 Update README to match implementation

---

## Testing

After implementation, verify:
```bash
# Test new flags
sp content list --category blog --status published
sp content search "AI" --source blog
sp content ingest /path --source test --dry-run
sp content popular --source blog --since 7d

# Test new command
sp content link delete <link-id> --yes
```
