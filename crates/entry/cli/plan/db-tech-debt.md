# DB Command Tech Debt - Phase 2

## Overview

This document addresses remaining friction points and enhancements for the `db` CLI command module to achieve full agent-friendliness and Rust standards compliance.

---

## Part 1: Rust Standards Violations

### 1.1 mod.rs Exceeds 300 Line Limit (593 lines)

**Current:** Single 593-line file
**Required:** ≤300 lines per Rust standards

**Solution:** Split into submodules:
```
db/
├── mod.rs           (~60 lines - enum + dispatcher)
├── types.rs         (output types - already exists)
├── query.rs         (~80 lines - query, execute)
├── schema.rs        (~120 lines - tables, describe, info, validate)
├── admin.rs         (~80 lines - migrate, assign-admin, status)
└── helpers.rs       (~30 lines - format_bytes, extract_relation_name)
```

---

## Part 2: Agent-Friendliness Issues

### 2.1 JSON Errors Not Structured

**Current:**
```bash
sp --json db query "SELECT * FROM nonexistent"
# Output: Error: Table or column not found: nonexistent  (plain text!)
```

**Required:**
```json
{
  "error": true,
  "code": "TABLE_NOT_FOUND",
  "message": "Table 'nonexistent' not found"
}
```

**Solution:** Create error wrapper that respects `--json` flag.

### 2.2 No Query Pagination Flags

**Current:** Must write SQL `LIMIT`/`OFFSET`
**Required:** Native flags

```rust
Query {
    sql: String,
    #[arg(long)]
    limit: Option<u32>,
    #[arg(long)]
    offset: Option<u32>,
}
```

### 2.3 No Table Filtering

**Current:** Lists all tables, no filtering
**Required:** Pattern matching

```rust
Tables {
    #[arg(long)]
    filter: Option<String>,
}
```

---

## Part 3: Missing Commands

### 3.1 db count

Quick row count without writing SQL.

```bash
sp db count users
sp --json db count users
```

**Output:**
```json
{"table": "users", "count": 47}
```

### 3.2 db indexes

List all indexes across all tables.

```bash
sp db indexes
sp db indexes --table users
sp --json db indexes
```

**Output:**
```json
{
  "indexes": [
    {"table": "users", "name": "users_pkey", "columns": ["id"], "unique": true}
  ],
  "total": 150
}
```

### 3.3 db size

Database and per-table size summary.

```bash
sp db size
sp --json db size
```

**Output:**
```json
{
  "database_size": "45.41 MB",
  "table_count": 85,
  "largest_tables": [
    {"name": "campaign_links", "size": "20.66 MB", "rows": 29305}
  ]
}
```

---

## Part 4: Implementation Plan

### Phase 1: Split mod.rs into Submodules
1. Create `helpers.rs` with utility functions
2. Create `query.rs` with query/execute commands
3. Create `schema.rs` with tables/describe/info/validate
4. Create `admin.rs` with migrate/assign-admin/status
5. Refactor `mod.rs` to be dispatcher only

### Phase 2: Add New Commands
1. Add `db count <table>` command
2. Add `db indexes [--table]` command
3. Add `db size` command

### Phase 3: Enhance Existing Commands
1. Add `--limit`/`--offset` to query
2. Add `--filter` to tables
3. Implement structured JSON errors

### Phase 4: Update Documentation
1. Update README with new commands
2. Add examples for new flags

---

## Checklist

- [ ] Split mod.rs into submodules (≤300 lines each)
- [ ] Create helpers.rs
- [ ] Create query.rs
- [ ] Create schema.rs
- [ ] Create admin.rs
- [ ] Add db count command
- [ ] Add db indexes command
- [ ] Add db size command
- [ ] Add --limit/--offset to query
- [ ] Add --filter to tables
- [ ] Implement structured JSON errors
- [ ] Update README
- [ ] Test all commands
